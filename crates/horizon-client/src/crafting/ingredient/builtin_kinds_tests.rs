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
fn item_model_component_literals_decode_validate_and_match() {
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:stick".into(),
        crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:item_model":"unregistered/model"})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"unregistered/model"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"minecraft:unregistered/model"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":":unregistered/model"}}"#,
    ] {
        let ingredient = parse(source, &items);
        assert_eq!(
            ingredient.shown.as_ref().unwrap()[0]
                .data()
                .unwrap()
                .other("minecraft:item_model"),
            Some(&Tag::String("minecraft:unregistered/model".into())),
        );
        let mut stack = ItemStack::new("minecraft:stick", 1);
        let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
            assert!(ingredient.test_in(&context, &stack));
        });
        assert_eq!(allocations.calls, 0, "{allocations:?}");
        for model in [
            "unregistered/model",
            "minecraft:unregistered/model",
            ":unregistered/model",
        ] {
            stack
                .data_mut()
                .set_loot_component("minecraft:item_model", &Some(Tag::String(model.into())));
            let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
                assert!(ingredient.test_in(&context, &stack));
            });
            assert_eq!(allocations.calls, 0, "{allocations:?}");
        }
        for model in [
            "minecraft:other",
            "fixture:unregistered/model",
            "Minecraft:unregistered/model",
        ] {
            stack
                .data_mut()
                .set_loot_component("minecraft:item_model", &Some(Tag::String(model.into())));
            assert!(!ingredient.test_in(&context, &stack));
        }
    }
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"Minecraft:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"minecraft:Stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"bad namespace:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"..:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"a:b:c"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":5}}"#,
    ] {
        let value: serde_json::Value = serde_json::from_str(source).unwrap();
        assert!(Ingredient::parse(&value, &items).is_none());
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(value["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(error.contains("component minecraft:item_model:"), "{error}");
    }
    for model in ["", ":", "minecraft:"] {
        assert_eq!(
            horizon_command::item::decoded_component_value(
                "minecraft:item_model",
                &Tag::String(model.into()),
                &items.component_registry,
            )
            .unwrap(),
            Tag::String("minecraft:".into()),
        );
    }
    for units in [vec![0xd800], vec![0xdc00], vec![0xd83d, 0xde00]] {
        let invalid_utf16 = Tag::String(horizon_core::nbt::JavaString(units));
        assert!(
            horizon_command::item::decoded_component_value(
                "minecraft:item_model",
                &invalid_utf16,
                &items.component_registry,
            )
            .is_err()
        );
    }
}

#[test]
fn recipes_component_cooking_decodes_keys_and_matches_without_allocations() {
    use crate::crafting::Recipes;
    use horizon_world::loot::PackOverlay;
    use serde_json::json;

    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:knowledge_book".into(),
        crate::json_nbt::json_to_compound(
            json!({"minecraft:recipes":["minecraft:unregistered/path","fixture:custom","unregistered/path","minecraft:"]})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let root = tempfile::tempdir().unwrap();
    let ingredient = |payload: serde_json::Value, strict| {
        json!({"type":"neoforge:components","items":"minecraft:knowledge_book",
            "components":{"minecraft:recipes":payload},"strict":strict})
    };
    let recipe = |ingredient: serde_json::Value| {
        json!({"type":"minecraft:smelting","ingredient":ingredient,
            "result":{"id":"minecraft:diamond"},"experience":0.0,"cookingtime":20})
    };
    let valid = ingredient(
        json!([
            "unregistered/path",
            "fixture:custom",
            ":unregistered/path",
            ""
        ]),
        false,
    );
    let mut sources = vec![("fixture:recipes".into(), recipe(valid.clone()))];
    for (index, payload) in [
        json!("unregistered/path"),
        json!({}),
        json!({"recipes":[]}),
        json!([{"id":"minecraft:stone"}]),
        json!([5]),
        json!([0.5]),
        json!(["valid", "Minecraft:invalid"]),
        json!(["minecraft:Uppercase"]),
        json!(["bad namespace:path"]),
        json!(["..:path"]),
        json!(["a:b:c"]),
        json!(["#minecraft:tag"]),
    ]
    .into_iter()
    .enumerate()
    {
        let invalid = ingredient(payload, false);
        assert!(Ingredient::parse(&invalid, &items).is_none());
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(invalid["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(error.contains("component minecraft:recipes:"), "{error}");
        sources.push((format!("fixture:refused_{index}"), recipe(invalid)));
    }
    let recipes =
        Recipes::from_sources(root.path(), &items, &PackOverlay::NONE, sources, &load).unwrap();
    let check = |recipes: &Recipes, stack: &ItemStack, expected| {
        // Build the cooking index once; only candidate matching is allocation-free.
        let _ = recipes.cook(&context, "minecraft:smelting", stack);
        let (found, allocations) = horizon_threads::allocation_probe::measure(|| {
            recipes.cook(&context, "minecraft:smelting", stack)
        });
        assert_eq!(allocations.calls, 0, "{allocations:?}");
        assert_eq!(found.is_some(), expected);
        if let Some(found) = found {
            assert_eq!(found.id, "fixture:recipes");
            assert_eq!(found.result.item.as_ref(), "minecraft:diamond");
        }
    };
    let mut stack = ItemStack::new("minecraft:knowledge_book", 1);
    check(&recipes, &stack, true);
    assert_eq!(
        recipes.cooking.len(),
        1,
        "invalid recipes alone are omitted"
    );
    let parsed = Ingredient::parse(&valid, &items).unwrap();
    assert_eq!(
        parsed.shown.as_ref().unwrap()[0]
            .data()
            .unwrap()
            .other("minecraft:recipes"),
        Some(&Tag::List(vec![
            Tag::String("minecraft:unregistered/path".into()),
            Tag::String("fixture:custom".into()),
            Tag::String("minecraft:unregistered/path".into()),
            Tag::String("minecraft:".into()),
        ])),
    );
    for (payload, matches) in [
        (
            json!([
                "unregistered/path",
                "fixture:custom",
                ":unregistered/path",
                ":"
            ]),
            true,
        ),
        (
            json!([
                "minecraft:unregistered/path",
                "fixture:custom",
                "unregistered/path",
                "minecraft:"
            ]),
            true,
        ),
        (
            json!([
                "fixture:custom",
                "unregistered/path",
                "unregistered/path",
                ""
            ]),
            false,
        ),
        (json!(["unregistered/path", "fixture:custom", ""]), false),
        (
            json!([
                "unregistered/path",
                "fixture:custom",
                "unregistered/path",
                "",
                "extra"
            ]),
            false,
        ),
        (
            json!([
                "unregistered/path",
                "fixture:other",
                "unregistered/path",
                ""
            ]),
            false,
        ),
        (
            json!([
                "Minecraft:unregistered/path",
                "fixture:custom",
                "unregistered/path",
                ""
            ]),
            false,
        ),
        (json!([]), false),
        (json!([{"id":"unregistered/path"}]), false),
    ] {
        let value = crate::json_nbt::json_to_tag(&payload).unwrap();
        stack
            .data_mut()
            .set_loot_component("minecraft:recipes", &Some(value));
        check(&recipes, &stack, matches);
    }
    let load_one = |ingredient| {
        Recipes::from_sources(
            root.path(),
            &items,
            &PackOverlay::NONE,
            vec![("fixture:recipes".into(), recipe(ingredient))],
            &load,
        )
        .unwrap()
    };
    stack
        .data_mut()
        .set_loot_component("minecraft:recipes", &None);
    let strict = load_one(ingredient(
        json!([
            "unregistered/path",
            "fixture:custom",
            "unregistered/path",
            "minecraft:"
        ]),
        true,
    ));
    check(&strict, &stack, true);
    stack.data_mut().set_other(
        "minecraft:custom_data",
        Some(Tag::compound([] as [(&str, Tag); 0])),
    );
    check(&strict, &stack, false);
    check(&recipes, &stack, true);
    stack.data_mut().set_other("minecraft:custom_data", None);
    stack.data_mut().set_other(
        "!minecraft:recipes",
        Some(Tag::compound([] as [(&str, Tag); 0])),
    );
    check(&recipes, &stack, false);
    let absent = load_one(
        json!({"type":"neoforge:components","items":"minecraft:knowledge_book",
        "components":{"!minecraft:recipes":{}}}),
    );
    check(&absent, &stack, true);
    stack.data_mut().set_other("!minecraft:recipes", None);
    check(&absent, &stack, false);
    let empty = load_one(ingredient(json!([]), false));
    check(&empty, &stack, false);
    for value in [
        Tag::List(vec![]),
        Tag::ByteArray(vec![]),
        Tag::IntArray(vec![]),
        Tag::LongArray(vec![]),
    ] {
        assert_eq!(
            horizon_command::item::decoded_component_value(
                "minecraft:recipes",
                &value,
                &items.component_registry,
            )
            .unwrap(),
            Tag::List(vec![]),
        );
        stack
            .data_mut()
            .set_loot_component("minecraft:recipes", &Some(value));
        check(&empty, &stack, true);
        check(&absent, &stack, false);
    }
    for value in [
        Tag::ByteArray(vec![0]),
        Tag::IntArray(vec![0]),
        Tag::LongArray(vec![0]),
        Tag::List(vec![Tag::String(horizon_core::nbt::JavaString(vec![
            0xd800,
        ]))]),
        Tag::List(vec![Tag::String(horizon_core::nbt::JavaString(vec![
            0xdc00,
        ]))]),
        Tag::List(vec![Tag::String(horizon_core::nbt::JavaString(vec![
            0xd83d, 0xde00,
        ]))]),
    ] {
        assert!(
            horizon_command::item::decoded_component_value(
                "minecraft:recipes",
                &value,
                &items.component_registry,
            )
            .is_err(),
        );
    }
    stack.count = 0;
    check(&empty, &stack, false);
}
