use super::*;
use horizon_core::nbt::Tag;
use horizon_world::native_load_context::{DataLoadOrigin, LoadContext, LoadInputs, LoopSide};

fn load() -> LoadContext {
    LoadContext::new(
        DataLoadOrigin::ServerDataLoad,
        std::sync::Arc::new(LoadInputs::new(Default::default(), Default::default())),
        [],
    )
}
fn parse(source: &str, items: &Items) -> Ingredient {
    Ingredient::parse(&serde_json::from_str(source).unwrap(), items).unwrap()
}
fn resources() -> Items {
    let root = std::path::PathBuf::from(
        std::env::var_os("HORIZON_TEST_RESOURCES").expect("prepared resources"),
    );
    let defaults = horizon_world::item_defaults::read(&root).unwrap();
    Items::catalog(&root, &horizon_world::loot::PackOverlay::NONE, &defaults).unwrap()
}
#[test]
#[ignore = "requires prepared resources"]
fn components_match_damage_ignore_count_and_refuse_empty() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5}}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut sword = ItemStack::new("minecraft:diamond_sword", 3);
    sword.damage = 5;
    assert!(ingredient.test_in(&context, &sword));
    sword.damage = 0;
    assert!(!ingredient.test_in(&context, &sword));
    sword.damage = 5;
    sword.count = 0;
    assert!(!ingredient.test_in(&context, &sword));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:air", 1)));
    assert!(!ingredient.is_simple());
}
#[test]
#[ignore = "requires prepared resources"]
fn components_strict_counts_max_damage_default_and_custom_name() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5},"strict":true}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
    sword.damage = 5;
    // MAX_DAMAGE is a real prototype component, absent from the literal patch.
    assert!(
        items.component_defaults["minecraft:diamond_sword"]
            .contains_key(&"minecraft:max_damage".into())
    );
    assert!(!ingredient.test_in(&context, &sword));
    sword.data_mut().set_loot_component(
        "minecraft:custom_name",
        &Some(Tag::String("Named sword".into())),
    );
    assert!(!ingredient.test_in(&context, &sword));

    // A literal supported-only prototype isolates strict extra-component behavior.
    let mut supported = Items::empty();
    supported.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([
        ("minecraft:diamond_sword".into(), crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:damage":0,"minecraft:max_damage":1561,"minecraft:max_stack_size":1}).as_object().unwrap()).unwrap())
    ]));
    let complete = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5,"minecraft:max_damage":1561,"minecraft:max_stack_size":1},"strict":true}"#,
        &supported,
    );
    let mut plain = ItemStack::new("minecraft:diamond_sword", 1);
    plain.damage = 5;
    assert!(complete.test_in(&context, &plain));
    assert!(!complete.test_in(&context, &sword));
    let missing_max = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5,"minecraft:max_stack_size":1},"strict":true}"#,
        &supported,
    );
    assert!(
        !missing_max.test_in(&context, &plain),
        "the literal max_damage default must be named"
    );
}
#[test]
#[ignore = "requires prepared resources"]
fn components_removal_requires_absence_including_defaults() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"!minecraft:enchantments":{}}}"#,
        &items,
    );
    assert!(!ingredient.shown.as_ref().unwrap()[0].carries_component(
        items.get("minecraft:diamond_sword"),
        "minecraft:enchantments"
    ));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
    assert!(!ingredient.test_in(&context, &sword));
    let patch = Tag::compound([("!minecraft:enchantments", Tag::Compound(Default::default()))]);
    (_, sword.extra) = crate::item_data::decode_components(Some(&patch));
    assert!(ingredient.test_in(&context, &sword));
    sword = ItemStack::new("minecraft:diamond_sword", 1);
    sword.data_mut().set_loot_component(
        "minecraft:enchantments",
        &Some(Tag::Compound(Default::default())),
    );
    assert!(!ingredient.test_in(&context, &sword));
}
#[test]
#[ignore = "requires prepared resources"]
fn block_tag_logs_uses_block_items_and_rejects_hash() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:block_tag","tag":"minecraft:logs"}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    assert!(ingredient.test_in(&context, &ItemStack::new("minecraft:oak_log", 1)));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:stone", 1)));
    assert!(ingredient.is_simple());
    let mut alias_items = items.clone();
    alias_items.block_tag_order.insert(
        "fixture:wall_signs".into(),
        vec!["minecraft:oak_wall_sign".into()],
    );
    let wall = parse(
        r#"{"type":"neoforge:block_tag","tag":"fixture:wall_signs"}"#,
        &alias_items,
    );
    assert_eq!(wall.order, ["minecraft:oak_sign"]);
    assert!(
        Ingredient::parse(
            &serde_json::json!({"type":"neoforge:block_tag","tag":"#minecraft:logs"}),
            &items
        )
        .is_none()
    );
}
#[test]
fn custom_display_matches_base_but_shows_another_item() {
    let items = Items::empty();
    let ingredient = parse(
        r#"{"type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    assert!(ingredient.test_in(&context, &ItemStack::new("minecraft:apple", 1)));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:diamond", 1)));
    assert!(ingredient.is_simple());
    assert_eq!(
        crate::recipe_book::Book::default().items_of(&ingredient),
        ["minecraft:diamond"]
    );
    assert_eq!(ingredient.order, ["minecraft:apple"]);
    let placement = crate::crafting::Placement::of(vec![Some(ingredient)], None);
    assert_eq!(placement.ingredients[0].order, ["minecraft:apple"]);
    // 26.3 TagSlotDisplay carries an item HolderSet, including the # prefix.
    let tags = Items::for_tests(std::collections::HashMap::from([(
        "minecraft:planks".into(),
        std::collections::HashSet::from(["minecraft:oak_planks".into()]),
    )]));
    let tagged = parse(
        r##"{"type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:tag","tag":"#minecraft:planks"}}"##,
        &tags,
    );
    assert_eq!(
        crate::recipe_book::Book::default().items_of(&tagged),
        ["minecraft:oak_planks"]
    );
}
#[test]
fn simplicity_follows_all_children_and_custom_display_base() {
    let items = Items::empty();
    for source in [
        r#"{"type":"neoforge:compound","children":["minecraft:apple","minecraft:stone"]}"#,
        r#"{"type":"neoforge:intersection","children":["minecraft:apple","minecraft:apple"]}"#,
        r#"{"type":"neoforge:difference","base":["minecraft:apple","minecraft:stone"],"subtracted":"minecraft:stone"}"#,
    ] {
        assert!(parse(source, &items).is_simple());
    }
    let source = r#"{"type":"neoforge:custom_display","base":{"type":"neoforge:components","items":"minecraft:apple","components":{}},"display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#;
    assert!(!parse(source, &items).is_simple());
    let mut custom = Ingredient::of(vec!["minecraft:apple".into()]);
    custom.custom = Some(("fixture:custom".into(), Default::default()));
    assert!(!custom.is_simple());
}

#[test]
fn builtin_kinds_bypass_mod_resolution_and_honor_type_key_priority() {
    let load = load();
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5}}"#,
        r#"{"type":"neoforge:block_tag","tag":"minecraft:logs"}"#,
        r#"{"type":"fixture:wrong","neoforge:ingredient_type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#,
    ] {
        let mut json: Value = serde_json::from_str(source).unwrap();
        let original = json.clone();
        assert!(!Ingredient::resolve(&mut json, &load));
        assert_eq!(json, original);
    }
}

#[test]
fn recipe_book_and_placement_use_component_ingredient_item_lists() {
    use crate::recipe_book::{Book, Category, Display, Entry};
    let items = Items::empty();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5}}"#,
        &items,
    );
    let entry = Entry {
        id: "fixture:components".into(),
        display_id: -1,
        category: Category::CraftingEquipment,
        group: None,
        display: Display::Shapeless(vec![ingredient.clone()]),
        result: ItemStack::new("minecraft:diamond", 1),
        variants: Vec::new(),
        stacks: Vec::new(),
    };
    let mut book = Book::default();
    let placement = crate::crafting::Placement::of(vec![Some(ingredient)], None);
    book.placements.insert(entry.id.clone(), placement.clone());
    let mut stock = crate::menu::place::Stacked::default();
    // An undamaged sword cannot satisfy the concrete recipe, but the book and
    // placement intentionally account only for its item holder.
    stock.account(&std::sync::Arc::from("minecraft:diamond_sword"), 1);
    assert!(book.can_craft(&entry, &stock));
    assert!(stock.can_craft(&placement.ingredients.iter().collect::<Vec<_>>()));
    assert_eq!(
        book.items_of(&placement.ingredients[0]),
        ["minecraft:diamond_sword"]
    );
}

#[test]
fn brewing_fuel_codec_compiles_recipes_and_borrows_live_values() {
    use horizon_core::nbt::JavaString;
    use serde_json::json;
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:blaze_powder".into(),
        [(
            "minecraft:brewing_fuel".into(),
            Tag::compound([
                ("uses", Tag::Byte(-2)),
                ("speed_multiplier", Tag::Double(1.25f64.to_bits())),
                ("ignored", Tag::Int(7)),
            ]),
        )]
        .into_iter()
        .collect(),
    )]));
    let ingredient_json = |payload: Value| {
        json!({
            "type":"neoforge:components", "items":"minecraft:blaze_powder",
            "components":{"minecraft:brewing_fuel":payload}
        })
    };
    let numeric = ingredient_json(json!({"uses":-2.75,"speed_multiplier":1.25,"ignored":false}));
    let reference =
        ingredient_json(json!({"uses":"missing/provider","speed_multiplier":":missing/speed"}));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let root = tempfile::tempdir().unwrap();
    let recipe = |ingredient: Value| {
        json!({
            "type":"minecraft:crafting_shapeless", "ingredients":[ingredient],
            "result":{"id":"minecraft:diamond","count":2}
        })
    };
    let mut sources = vec![
        ("fixture:numeric".into(), recipe(numeric.clone())),
        ("fixture:reference".into(), recipe(reference.clone())),
    ];
    for (n, payload) in [
        json!({}),
        json!({"uses":1}),
        json!({"speed_multiplier":1}),
        json!({"uses":{},"speed_multiplier":1}),
        json!({"uses":1,"speed_multiplier":[]}),
        json!({"uses":"UPPER:key","speed_multiplier":1}),
        json!({"uses":1,"speed_multiplier":"..:key"}),
    ]
    .into_iter()
    .enumerate()
    {
        let invalid = ingredient_json(payload);
        assert!(Ingredient::parse(&invalid, &items).is_none());
        sources.push((format!("fixture:invalid_{n}"), recipe(invalid)));
    }
    let recipes = crate::crafting::Recipes::from_sources(
        root.path(),
        &items,
        &horizon_world::loot::PackOverlay::NONE,
        sources,
        &load,
    )
    .unwrap();
    assert_eq!(recipes.crafting.len(), 2);
    let mut stack = ItemStack::new("minecraft:blaze_powder", 1);
    let (id, result) = recipes.find(&context, &[Some(stack.clone())], 1).unwrap();
    assert_eq!(id, "fixture:numeric");
    assert_eq!(result.item.as_ref(), "minecraft:diamond");
    assert_eq!(result.count, 2);
    let ingredient = Ingredient::parse(&numeric, &items).unwrap();
    assert!(ingredient.test_in(&context, &stack));
    stack.data_mut().set_other(
        "minecraft:brewing_fuel",
        Some(Tag::compound([
            ("uses", Tag::Long(4_294_967_294)),
            ("speed_multiplier", Tag::Float(1.25f32.to_bits())),
            ("ignored", Tag::String("different".into())),
        ])),
    );
    assert_eq!(
        recipes.find(&context, &[Some(stack.clone())], 1).unwrap().0,
        "fixture:numeric"
    );
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            assert!(std::hint::black_box(
                ingredient.test_in(&context, std::hint::black_box(&stack))
            ));
        }
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(allocations.bytes, 0);
    stack.data_mut().set_other(
        "minecraft:brewing_fuel",
        Some(Tag::compound([
            ("uses", Tag::String("minecraft:missing/provider".into())),
            ("speed_multiplier", Tag::String("missing/speed".into())),
        ])),
    );
    assert_eq!(
        recipes.find(&context, &[Some(stack.clone())], 1).unwrap().0,
        "fixture:reference"
    );
    let reference = Ingredient::parse(&reference, &items).unwrap();
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        assert!(reference.test_in(&context, &stack));
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    for malformed in [
        Tag::compound([("uses", Tag::Int(-2))]),
        Tag::compound([
            ("uses", Tag::Int(-2)),
            ("speed_multiplier", Tag::String(JavaString(vec![0xd800]))),
        ]),
        Tag::compound([
            ("uses", Tag::Int(-2)),
            ("speed_multiplier", Tag::String("UPPER".into())),
        ]),
    ] {
        stack
            .data_mut()
            .set_other("minecraft:brewing_fuel", Some(malformed));
        assert!(!ingredient.test_in(&context, &stack));
        assert!(recipes.find(&context, &[Some(stack.clone())], 1).is_none());
    }
    let decode = |uses, speed| {
        horizon_command::item::decoded_component_value(
            "minecraft:brewing_fuel",
            &Tag::compound([("uses", uses), ("speed_multiplier", speed)]),
            &items.component_registry,
        )
        .unwrap()
    };
    for (input, expected) in [
        (Tag::Byte(-2), -2),
        (Tag::Short(-2), -2),
        (Tag::Int(-2), -2),
        (Tag::Long(4_294_967_294), -2),
        (Tag::Float((-2.75f32).to_bits()), -2),
        (Tag::Double(f64::NAN.to_bits()), 0),
        (Tag::Double(f64::INFINITY.to_bits()), i32::MAX),
        (Tag::Double(f64::NEG_INFINITY.to_bits()), i32::MIN),
    ] {
        assert_eq!(
            decode(input, Tag::Double(1.25f64.to_bits())),
            Tag::compound([
                ("uses", Tag::Int(expected)),
                ("speed_multiplier", Tag::Float(1.25f32.to_bits())),
            ])
        );
    }
    for (input, expected) in [
        (Tag::Byte(-1), (-1.0f32).to_bits()),
        (Tag::Short(2), 2f32.to_bits()),
        (Tag::Int(16_777_217), 16_777_216f32.to_bits()),
        (Tag::Long(i64::MAX), (i64::MAX as f32).to_bits()),
        (
            Tag::Double(f64::INFINITY.to_bits()),
            f32::INFINITY.to_bits(),
        ),
    ] {
        assert_eq!(
            decode(Tag::Int(1), input),
            Tag::compound([
                ("uses", Tag::Int(1)),
                ("speed_multiplier", Tag::Float(expected)),
            ])
        );
    }
    let nan = decode(Tag::Int(1), Tag::Float(0x7f80_0001));
    // NaN payload bits are not part of boxed-float equality.
    assert!(matches!(
        nan.get("speed_multiplier"),
        Some(Tag::Float(value)) if f32::from_bits(*value).is_nan()
    ));
    for (expected, actual, matches) in [
        (0f32.to_bits(), (-0f32).to_bits(), false),
        (f32::NAN.to_bits(), 0xff80_0001, true),
        (f32::INFINITY.to_bits(), f32::INFINITY.to_bits(), true),
    ] {
        let raw = Tag::compound([
            ("uses", Tag::Int(1)),
            ("speed_multiplier", Tag::Float(expected)),
        ]);
        let mut a = ItemStack::new("minecraft:blaze_powder", 1);
        a.data_mut().set_other("minecraft:brewing_fuel", Some(raw));
        stack.data_mut().set_other(
            "minecraft:brewing_fuel",
            Some(Tag::compound([
                ("uses", Tag::Int(1)),
                ("speed_multiplier", Tag::Float(actual)),
            ])),
        );
        assert_eq!(
            horizon_items::item_data::ingredient_components::component_equal(
                "minecraft:brewing_fuel",
                &a,
                &stack,
            ),
            matches
        );
    }
}
