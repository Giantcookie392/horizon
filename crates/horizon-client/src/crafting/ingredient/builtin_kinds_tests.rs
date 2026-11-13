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
fn enchantment_glint_override_decodes_boolean_literals_and_borrowed_numeric_values() {
    const COMPONENT: &str = "minecraft:enchantment_glint_override";
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:apple".into(),
        std::collections::BTreeMap::from([(COMPONENT.into(), Tag::Short(256))]),
    )]));
    let enabled = parse(
        r#"{"type":"neoforge:components","items":"minecraft:apple","components":{"minecraft:enchantment_glint_override":true}}"#,
        &items,
    );
    let disabled = parse(
        r#"{"type":"neoforge:components","items":"minecraft:apple","components":{"minecraft:enchantment_glint_override":false}}"#,
        &items,
    );
    for (ingredient, expected) in [(&enabled, Tag::Byte(1)), (&disabled, Tag::Byte(0))] {
        assert_eq!(
            ingredient.shown.as_ref().unwrap()[0]
                .data()
                .unwrap()
                .other(COMPONENT),
            Some(&expected),
        );
    }
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut candidate = ItemStack::new("minecraft:apple", 1);
    assert!(
        enabled.test_in(&context, &candidate),
        "inherited numeric default"
    );
    assert!(!disabled.test_in(&context, &candidate));

    // NbtOps.getBooleanValue uses Number.doubleValue, not Tag.asBoolean's byteValue.
    for (value, expected) in [
        (Tag::Byte(-1), true),
        (Tag::Short(256), true),
        (Tag::Int(256), true),
        (Tag::Long(4_294_967_296), true),
        (Tag::Float(0.5_f32.to_bits()), true),
        (Tag::Double(0.5_f64.to_bits()), true),
        (Tag::Float(f32::NAN.to_bits()), true),
        (Tag::Double(f64::NAN.to_bits()), true),
        (Tag::Double(f64::INFINITY.to_bits()), true),
        (Tag::Double(f64::from_bits(1).to_bits()), true),
        (Tag::Byte(0), false),
        (Tag::Short(0), false),
        (Tag::Int(0), false),
        (Tag::Long(0), false),
        (Tag::Float((-0.0_f32).to_bits()), false),
        (Tag::Double((-0.0_f64).to_bits()), false),
    ] {
        assert_eq!(
            horizon_command::item::decoded_component_value(
                COMPONENT,
                &value,
                &items.component_registry,
            )
            .unwrap(),
            Tag::Byte(i8::from(expected)),
        );
        candidate
            .data_mut()
            .set_loot_component(COMPONENT, &Some(value));
        assert_eq!(enabled.test_in(&context, &candidate), expected);
        assert_eq!(disabled.test_in(&context, &candidate), !expected);
    }
    let numeric = parse(
        r#"{"type":"neoforge:components","items":"minecraft:apple","components":{"minecraft:enchantment_glint_override":0.5}}"#,
        &items,
    );
    candidate
        .data_mut()
        .set_loot_component(COMPONENT, &Some(Tag::Int(256)));
    assert!(numeric.test_in(&context, &candidate));
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            assert!(enabled.test_in(&context, std::hint::black_box(&candidate)));
        }
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(allocations.bytes, 0);
    candidate.data_mut().set_loot_component(COMPONENT, &None);
    assert!(
        enabled.test_in(&context, &candidate),
        "default is inherited again"
    );
    candidate.data_mut().set_loot_component(
        "!minecraft:enchantment_glint_override",
        &Some(Tag::Compound(Default::default())),
    );
    assert!(
        !enabled.test_in(&context, &candidate),
        "removed, not inherited"
    );
    assert!(
        !disabled.test_in(&context, &candidate),
        "absent is not false"
    );

    let invalid: Value = serde_json::from_str(
        r#"{"type":"neoforge:components","items":"minecraft:apple","components":{"minecraft:enchantment_glint_override":"true"}}"#,
    )
    .unwrap();
    assert!(Ingredient::parse(&invalid, &items).is_none());
    let rule = IngredientRule::Components {
        patch: crate::json_nbt::json_to_compound(invalid["components"].as_object().unwrap())
            .unwrap(),
        removed: Vec::new(),
        strict: false,
    };
    assert_eq!(
        components::validate(&rule, &items).unwrap_err(),
        "component minecraft:enchantment_glint_override: Not a number",
    );
}

#[test]
fn food_codec_compiles_recipes_and_matches_borrowed_values() {
    use horizon_world::loot::PackOverlay;
    use serde_json::json;
    const COMPONENT: &str = "minecraft:food";
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:apple".into(),
        std::collections::BTreeMap::from([(
            COMPONENT.into(),
            Tag::compound([
                ("nutrition", Tag::Long(4_294_967_300)),
                ("saturation", Tag::Double((-2.5_f64).to_bits())),
                ("ignored", Tag::Int(99)),
            ]),
        )]),
    )]));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let payload = json!({"nutrition":4.9,"saturation":-2.5,"effects":"ignored legacy field"});
    let ingredient = |food: Value| {
        json!({"type":"neoforge:components","items":"minecraft:apple",
            "components":{"minecraft:food":food}})
    };
    let recipe = |food: Value| {
        json!({"type":"minecraft:crafting_shaped","pattern":["A"],
            "key":{"A":ingredient(food)},"result":{"id":"minecraft:diamond"}})
    };
    let mut sources = vec![("fixture:food".into(), recipe(payload.clone()))];
    for (index, invalid) in [
        json!({}),
        json!({"nutrition":4}),
        json!({"saturation":0}),
        json!({"nutrition":-1,"saturation":0}),
        json!({"nutrition":4,"saturation":"0"}),
        json!({"nutrition":4,"saturation":0,"can_always_eat":"false"}),
        json!([]),
    ]
    .into_iter()
    .enumerate()
    {
        let value = ingredient(invalid.clone());
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(value["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        assert!(
            components::validate(&rule, &items)
                .unwrap_err()
                .starts_with("component minecraft:food: "),
        );
        sources.push((format!("fixture:invalid_food_{index}"), recipe(invalid)));
    }
    let recipes = crate::crafting::Recipes::from_sources(
        &std::env::temp_dir(),
        &items,
        &PackOverlay::NONE,
        sources,
        &load,
    )
    .unwrap();
    assert_eq!(recipes.crafting.len(), 1);
    assert_eq!(recipes.crafting[0].0, "fixture:food");
    let parsed = Ingredient::parse(&ingredient(payload), &items).unwrap();
    assert_eq!(
        parsed.shown.as_ref().unwrap()[0]
            .data()
            .unwrap()
            .other(COMPONENT),
        Some(&Tag::compound([
            ("nutrition", Tag::Int(4)),
            ("saturation", Tag::Float((-2.5_f32).to_bits())),
            ("can_always_eat", Tag::Byte(0)),
        ])),
    );
    let mut grid = [Some(ItemStack::new("minecraft:apple", 1))];
    assert!(recipes.recipe_matches(&context, "fixture:food", &grid, 1));
    for (nutrition, saturation, always, expected) in [
        (Tag::Float(4.9_f32.to_bits()), Tag::Short(-2), None, false),
        (
            Tag::Double(4.9_f64.to_bits()),
            Tag::Double((-2.5_f64).to_bits()),
            None,
            true,
        ),
        (
            Tag::Byte(4),
            Tag::Float((-2.5_f32).to_bits()),
            Some(Tag::Int(256)),
            false,
        ),
        (
            Tag::Short(4),
            Tag::Float((-2.5_f32).to_bits()),
            Some(Tag::Double((-0.0_f64).to_bits())),
            true,
        ),
        (
            Tag::Long(4_294_967_300),
            Tag::Float((-2.5_f32).to_bits()),
            Some(Tag::Byte(0)),
            true,
        ),
    ] {
        let mut fields = vec![("nutrition", nutrition), ("saturation", saturation)];
        if let Some(always) = always {
            fields.push(("can_always_eat", always));
        }
        grid[0]
            .as_mut()
            .unwrap()
            .data_mut()
            .set_other(COMPONENT, Some(Tag::compound(fields)));
        assert_eq!(
            recipes.recipe_matches(&context, "fixture:food", &grid, 1),
            expected
        );
    }
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            assert!(parsed.test_in(&context, std::hint::black_box(grid[0].as_ref().unwrap())));
        }
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(allocations.bytes, 0);
    grid[0]
        .as_mut()
        .unwrap()
        .data_mut()
        .set_loot_component(COMPONENT, &None);
    grid[0]
        .as_mut()
        .unwrap()
        .data_mut()
        .set_loot_component("!minecraft:food", &Some(Tag::Compound(Default::default())));
    assert!(!recipes.recipe_matches(&context, "fixture:food", &grid, 1));

    // The direct float codec allows nonfinite values; record equality uses
    // Float.compare (canonical NaNs and distinct signed zero), not Rust ==.
    for (a, b, equal) in [
        (f32::NAN.to_bits(), 0x7f800001, true),
        (0.0_f32.to_bits(), (-0.0_f32).to_bits(), false),
        (f32::INFINITY.to_bits(), f32::INFINITY.to_bits(), true),
    ] {
        let stack = |bits| {
            let mut stack = ItemStack::new("minecraft:apple", 1);
            stack.data_mut().set_other(
                COMPONENT,
                Some(Tag::compound([
                    ("nutrition", Tag::Int(4)),
                    ("saturation", Tag::Float(bits)),
                ])),
            );
            stack
        };
        let a = stack(a);
        let b = stack(b);
        assert_eq!(
            horizon_items::item_data::ingredient_components::component_equal(COMPONENT, &a, &b),
            equal,
        );
        assert!(
            horizon_command::item::decoded_component_value(
                COMPONENT,
                a.data().unwrap().other(COMPONENT).unwrap(),
                &items.component_registry,
            )
            .is_ok()
        );
    }
    let numeric = Tag::compound([
        ("nutrition", Tag::Double(f64::NAN.to_bits())),
        ("saturation", Tag::Long(16_777_217)),
        ("can_always_eat", Tag::Double(f64::NAN.to_bits())),
        ("obsolete", Tag::String("discarded".into())),
    ]);
    assert_eq!(
        horizon_command::item::decoded_component_value(
            COMPONENT,
            &numeric,
            &items.component_registry,
        )
        .unwrap(),
        Tag::compound([
            ("nutrition", Tag::Int(0)),
            ("saturation", Tag::Float(16_777_216.0_f32.to_bits())),
            ("can_always_eat", Tag::Byte(1)),
        ]),
    );
}
