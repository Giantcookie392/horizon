# Custom ingredient kinds

Implement `Hooks::ingredient(context, event)` to resolve recipe ingredient types outside the `minecraft:` namespace. Fill `event.items` with the item IDs your ingredient accepts. The loader removes duplicates in first-seen order.

This list supplies recipe-book placement candidates. Use the [match-time hook](ingredient-matching.md) to check a concrete stack. The list must cover every item the stack test can accept: the test may widen beyond the list for shaped and non-simple shapeless matching, but everything that reads the list (recipe book, placement, recipe property sets, vanilla clients) sees only the list. A nonempty candidate list also filters matching by item; an empty custom list retains the predicate without this filter.

## Load contract

- The hook receives the owning load's `DataLoadContext`.
- Each occurrence is dispatched once per load, including nested ingredients and ingredients in any recipe slot.
- Dispatch occurs before the load's tables parse recipes. Server tables, stations and copies sent to guests share that load.
- The client's recipe preload reports `ClientPreload` through `origin()`; the server's recipe load reports `ServerDataLoad`, including on a dedicated server.
- `/reload` dispatches the occurrences again. A client joining a remote server does not receive `ServerDataLoad`.
- Return a consistent answer for the same input. An empty custom list retains the recipe and its match-time predicate, with a warning that the recipe cannot be placed. It is excluded from recipe-book learning/displays, recipe property sets and stonecutter selections; the server also refuses recipe-book placement. The effective recipe map and manual matcher still retain it.
- An existing empty item tag likewise retains an unplaceable recipe, but never matches a stack. An undefined tag fails the recipe with an error. A direct empty list (`[]`) remains invalid and fails the recipe with an error.

## Ingredient data

Read `event.data` as an API `Compound`, using typed `Tag` variants such as `Int`, `Long`, `List` and `Compound`, not JSON accessors.

The loader converts recipe metadata once using vanilla JsonOps-to-NbtOps rules. JSON `true` and `false` become `Byte` 1 and 0, so the hook cannot tell a bool from the number 1 or 0. An integer becomes the smallest of `Byte`, `Short`, `Int` and `Long` that holds it (`1` is a `Byte`, `300` a `Short`, `100000` an `Int`). A decimal becomes a `Float` if it survives a round trip through `f32` (`0.5`), otherwise a `Double` (`0.1`). Numeric JSON arrays remain lists. Nested null values refuse the ingredient and fail the recipe. The match-time hook receives the same retained compound without reconversion.
