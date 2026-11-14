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
fn custom_model_data_recipe_decodes_ordered_lists_and_matches_without_allocating() {
    use crate::crafting::{Recipes, ResolvedRecipe, ResolvedSources};
    use horizon_core::nbt::JavaString;
    use horizon_world::loot::PackOverlay;
    use serde_json::json;
    use std::sync::Arc;

    let items = Items::empty();
    let payload = json!({
        "floats": [16777217, -0.0, 1e40],
        "flags": [0, 0.25, -2],
        "strings": ["", "😀"],
        "colors": [[1, 0, 0], 4294901760i64],
        "ignored": {"arbitrary": true}
    });
    let source = json!({
        "type": "neoforge:components", "items": "minecraft:stone",
        "components": {"minecraft:custom_model_data": payload}
    });
    let ingredient = Ingredient::parse(&source, &items).unwrap();
    let expected = Tag::compound([
        (
            "floats",
            Tag::List(vec![
                Tag::Float(16777216.0f32.to_bits()),
                Tag::Float(0), // JsonOps canonicalizes the integer-valued -0.0 spelling.
                Tag::Float(0x7f800000),
            ]),
        ),
        (
            "flags",
            Tag::List(vec![Tag::Byte(0), Tag::Byte(1), Tag::Byte(1)]),
        ),
        (
            "strings",
            Tag::List(vec![
                Tag::String("".into()),
                Tag::String(JavaString(vec![0xd83d, 0xde00])),
            ]),
        ),
        (
            "colors",
            Tag::List(vec![Tag::Int(-65536), Tag::Int(-65536)]),
        ),
    ]);
    assert_eq!(
        ingredient.shown.as_ref().unwrap()[0]
            .data()
            .unwrap()
            .other("minecraft:custom_model_data"),
        Some(&expected),
    );
    let named = |name: &str, ingredient: serde_json::Value| {
        let file = Arc::new(json!({
            "type": "minecraft:crafting_shaped", "pattern": ["A"],
            "key": {"A": ingredient}, "result": {"id": "minecraft:diamond", "count": 2}
        }));
        ResolvedRecipe {
            name: name.into(),
            file: file.clone(),
            resolved: file,
        }
    };
    let mut sources = ResolvedSources {
        recipes: vec![named("fixture:model", source.clone())],
        ..ResolvedSources::default()
    };
    for (index, invalid) in [
        json!(3),
        json!({"floats": 1}),
        json!({"floats": ["1"]}),
        json!({"flags": ["true"]}),
        json!({"strings": [1]}),
        json!({"colors": [[1, 0]]}),
        json!({"colors": ["red"]}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut bad = source.clone();
        bad["components"]["minecraft:custom_model_data"] = invalid;
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(bad["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(error.contains("minecraft:custom_model_data"), "{error}");
        sources
            .recipes
            .push(named(&format!("fixture:invalid_{index}"), bad));
    }
    let recipes =
        Recipes::from_resolved(&std::env::temp_dir(), &items, &PackOverlay::NONE, &sources)
            .unwrap();
    assert_eq!(recipes.crafting.len(), 1);
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut stack = ItemStack::new("minecraft:stone", 1);
    stack
        .data_mut()
        .set_other("minecraft:custom_model_data", Some(expected.clone()));
    assert!(recipes.recipe_matches(&context, "fixture:model", &[Some(stack.clone())], 1));
    assert_eq!(
        recipes.by_type["minecraft:crafting_shaped"][0].results[0].count,
        2
    );
    // Raw live numeric widths and RGB vectors compare to the canonical recipe value.
    let mut raw = expected.clone();
    if let Tag::Compound(fields) = &mut raw {
        fields.insert("flags".into(), Tag::LongArray(vec![0, 23, -1]));
        fields.insert(
            "colors".into(),
            Tag::List(vec![Tag::ByteArray(vec![1, 0, 0]), Tag::Long(4294901760)]),
        );
        fields.insert("unknown".into(), Tag::String("ignored".into()));
    }
    stack
        .data_mut()
        .set_other("minecraft:custom_model_data", Some(raw));
    assert!(recipes.recipe_matches(&context, "fixture:model", &[Some(stack.clone())], 1));
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            assert!(std::hint::black_box(ingredient.test_in(&context, &stack)));
        }
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(allocations.bytes, 0);
    for (field, different) in [
        (
            "floats",
            Tag::List(vec![
                Tag::Float(16777216.0f32.to_bits()),
                Tag::Float(0x80000000),
                Tag::Float(0x7f800000),
            ]),
        ),
        ("flags", Tag::ByteArray(vec![1, 0, 1])),
        (
            "strings",
            Tag::List(vec![Tag::String("😀".into()), Tag::String("".into())]),
        ),
        ("colors", Tag::IntArray(vec![16711680, -65536])),
    ] {
        let mut value = expected.clone();
        if let Tag::Compound(fields) = &mut value {
            fields.insert(field.into(), different);
        }
        stack
            .data_mut()
            .set_other("minecraft:custom_model_data", Some(value));
        assert!(
            !recipes.recipe_matches(&context, "fixture:model", &[Some(stack.clone())], 1),
            "{field}"
        );
    }
    let empty = parse(
        r#"{"type":"neoforge:components","items":"minecraft:stone","components":{"minecraft:custom_model_data":{}}}"#,
        &items,
    );
    stack.data_mut().set_other(
        "minecraft:custom_model_data",
        Some(Tag::compound([
            ("floats", Tag::IntArray(vec![])),
            ("flags", Tag::ByteArray(vec![])),
        ])),
    );
    assert!(empty.test_in(&context, &stack));
    assert!(!empty.test_in(&context, &ItemStack::new("minecraft:stone", 1)));
    // Unpaired UTF-16 surrogates never become replacement characters.
    let special = |units| {
        Tag::compound([
            ("strings", Tag::List(vec![Tag::String(JavaString(units))])),
            ("floats", Tag::List(vec![Tag::Float(0x7fc01234)])),
        ])
    };
    let a = special(vec![0xd800]);
    let canonical = horizon_command::item::decoded_component_value(
        "minecraft:custom_model_data",
        &a,
        &items.component_registry,
    )
    .unwrap();
    assert!(matches!(
        canonical.get("floats"),
        Some(Tag::List(values)) if matches!(
            values.as_slice(),
            [Tag::Float(value)] if f32::from_bits(*value).is_nan()
        )
    ));
    assert!(horizon_items::item_data::custom_model_data::equal(
        &a, &canonical
    ));
    assert!(!horizon_items::item_data::custom_model_data::equal(
        &a,
        &special(vec![0xfffd])
    ));
    for number in [
        Tag::Byte(-1),
        Tag::Short(-1),
        Tag::Int(-1),
        Tag::Long(-1),
        Tag::Float((-1.0f32).to_bits()),
        Tag::Double((-1.0f64).to_bits()),
    ] {
        let value = Tag::compound([("floats", Tag::List(vec![number]))]);
        assert_eq!(
            horizon_command::item::decoded_component_value(
                "minecraft:custom_model_data",
                &value,
                &items.component_registry,
            )
            .unwrap()
            .get("floats"),
            Some(&Tag::List(vec![Tag::Float(0xbf800000)])),
        );
    }
}

#[test]
fn components_dyed_color_decodes_rgb_forms_and_matches_borrowed_values() {
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:leather_helmet".into(),
        crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:dyed_color":[1,0,0]})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let integer = parse(
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":-65536}}"#,
        &items,
    );
    let vector = parse(
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":[1.0,0.0,0.0]}}"#,
        &items,
    );
    assert_eq!(
        vector.shown.as_ref().unwrap()[0].data().unwrap().dyed_color,
        Some(-65536)
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let default = ItemStack::new("minecraft:leather_helmet", 1);
    let mut typed = default.clone();
    typed.data_mut().dyed_color = Some(-65536);
    let mut raw = default.clone();
    raw.data_mut().set_other(
        "minecraft:dyed_color",
        Some(Tag::List(vec![Tag::Int(1), Tag::Int(0), Tag::Int(0)])),
    );
    for ingredient in [&integer, &vector] {
        assert!(ingredient.test_in(&context, &default));
        assert!(ingredient.test_in(&context, &typed));
        assert!(ingredient.test_in(&context, &raw));
    }
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            for ingredient in [&integer, &vector] {
                for stack in [&default, &typed, &raw] {
                    assert!(std::hint::black_box(ingredient.test_in(&context, stack)));
                }
            }
        }
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(allocations.bytes, 0);
    // RGB equality retains the integer's alpha bits; rendering opacity is unrelated.
    typed.data_mut().dyed_color = Some(16711680);
    assert!(!integer.test_in(&context, &typed));
    typed.data_mut().dyed_color = Some(-16776961);
    assert!(!vector.test_in(&context, &typed));
    raw.data_mut().set_other(
        "minecraft:dyed_color",
        Some(Tag::String("not a color".into())),
    );
    assert!(!integer.test_in(&context, &raw));

    // All boxed number widths and NbtOps collection forms reach the same codec.
    for (value, expected) in [
        (Tag::Byte(-1), -1),
        (Tag::Short(-32768), -32768),
        (Tag::Int(305419896), 305419896),
        (Tag::Long(4294901760), -65536),
        (Tag::Float((-65536.5f32).to_bits()), -65536),
        (Tag::Double((-65536.5f64).to_bits()), -65536),
        (Tag::ByteArray(vec![1, 0, 0]), -65536),
        (Tag::IntArray(vec![1, 0, 0]), -65536),
        (Tag::LongArray(vec![1, 0, 0]), -65536),
        (
            Tag::List(vec![
                Tag::Double(0.5f64.to_bits()),
                Tag::Double(0.0f64.to_bits()),
                Tag::Double(0.0f64.to_bits()),
            ]),
            -8454144,
        ),
        // Channels have no 0..1 restriction, and floor before byte masking.
        (Tag::ByteArray(vec![255, 2, 0]), 0xff01_fe00u32 as i32),
        (
            Tag::List(vec![
                Tag::Float(f32::NAN.to_bits()),
                Tag::Float(f32::INFINITY.to_bits()),
                Tag::Float(f32::NEG_INFINITY.to_bits()),
            ]),
            0xff00_ff00u32 as i32,
        ),
    ] {
        assert_eq!(
            horizon_command::item::decoded_component_value(
                "minecraft:dyed_color",
                &value,
                &items.component_registry,
            )
            .unwrap(),
            Tag::Int(expected)
        );
        let patch = Tag::compound([("minecraft:dyed_color", value)]);
        let (_, data) = crate::item_data::decode_components(Some(&patch));
        assert_eq!(data.unwrap().dyed_color, Some(expected));
    }

    for source in [
        r##"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":"#ff0000"}}"##,
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":[1,0]}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":[1,0,0,1]}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":[1,"red",0]}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":{"rgb":16711680,"show_in_tooltip":true}}}"#,
    ] {
        let json: serde_json::Value = serde_json::from_str(source).unwrap();
        assert!(Ingredient::parse(&json, &items).is_none());
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(json["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(error.contains("minecraft:dyed_color"), "{error}");
    }
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
