use super::*;

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

