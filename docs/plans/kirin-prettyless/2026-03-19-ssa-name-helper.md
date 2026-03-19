# Extract `ssa_name()` Helper on Document

## Problem

The same SSA name resolution pattern is duplicated 7 times across `kirin-prettyless`:

1. `impls.rs:21-24` -- `ResultValue::namespaced_pretty_print`
2. `impls.rs:37-40` -- `ResultValue::pretty_print_name`
3. `impls.rs:67-70` -- `SSAValue::namespaced_pretty_print`
4. `impls.rs:83-86` -- `SSAValue::pretty_print_name`
5. `document/ir_render.rs:52-57` -- block argument names
6. `document/ir_render.rs:114-119` -- port names in `print_ports`
7. `document/ir_render.rs:178-183` -- yield SSA names in `print_digraph`

The pattern is always:
```rust
if let Some(name_sym) = info.name() {
    stage.symbol_table().resolve(name_sym)
        .cloned() // or not, depending on context
        .unwrap_or_else(|| format!("{}", Id::from(ssa).raw()))
} else {
    format!("{}", Id::from(ssa).raw())
}
```

In `impls.rs`, the variant is slightly different (returns `doc.text(format!("%{}", resolved_name))` directly), but the core logic is the same: look up name symbol, resolve to string, fall back to numeric ID.

## Research Findings

### Pattern in `impls.rs` (ResultValue / SSAValue impls)

Both `ResultValue` and `SSAValue` have identical `namespaced_pretty_print` and `pretty_print_name` implementations. The pattern:
```rust
let info = self.expect_info(doc.stage());
if let Some(name) = info.name()
    && let Some(resolved_name) = doc.stage().symbol_table().resolve(name)
{
    return doc.text(format!("%{}", resolved_name));
}
doc.text(self.to_string())
```

The `pretty_print_type` methods are also identical between the two types.

### Pattern in `ir_render.rs`

In `print_block` (line 52), `print_ports` (line 114), and `print_digraph` yield (line 178):
```rust
let name = if let Some(name_sym) = info.name() {
    self.stage.symbol_table().resolve(name_sym)
        .cloned()
        .unwrap_or_else(|| format!("{}", Id::from(ssa).raw()))
} else {
    format!("{}", Id::from(ssa).raw())
};
```

This is used to produce `%name` text in various contexts.

### Key Difference

The `impls.rs` pattern converts from `SSAValue`/`ResultValue` and their `Display` impl includes the `%` prefix. The `ir_render.rs` pattern works with `SSAValue`/`Port`/`BlockArgument` and adds `%` manually.

## Proposed Design

Add an `ssa_name` method on `Document` that resolves an SSA value's name:

```rust
impl<'a, L: Dialect + PrettyPrint> Document<'a, L>
where
    L::Type: std::fmt::Display,
{
    /// Resolve the display name of an SSA value.
    ///
    /// Returns the symbol-table name if one exists, otherwise falls back
    /// to the numeric ID.
    pub fn ssa_name<V>(&self, value: V) -> String
    where
        V: GetInfo<L>,           // provides expect_info(stage)
        Id: From<V>,             // provides numeric fallback
    {
        let info = value.expect_info(self.stage);
        if let Some(name_sym) = info.name() {
            if let Some(resolved) = self.stage.symbol_table().resolve(name_sym) {
                return resolved.clone();
            }
        }
        format!("{}", Id::from(value).raw())
    }
}
```

This returns just the bare name (no `%` prefix), since callers differ on whether they prepend `%` or `^`. Callers format as `format!("%{}", doc.ssa_name(value))`.

### Consolidating ResultValue/SSAValue PrettyPrint impls

The `ResultValue` and `SSAValue` impls are fully identical. Two approaches:

**Option A: Generic helper methods.** Add private methods on `Document` that take anything implementing `GetInfo<L> + Into<Id> + Display`:
```rust
fn print_ssa_ref<V: GetInfo<L> + Display>(&'a self, value: &V) -> ArenaDoc<'a> { ... }
fn print_ssa_type<V: GetInfo<L>>(&'a self, value: &V) -> ArenaDoc<'a> { ... }
```
Then both `ResultValue` and `SSAValue` impls delegate to these.

**Option B: Macro.** A small `impl_ssa_pretty_print!` macro that generates the identical impl for both types. Less principled but concise.

Recommend Option A since it follows the project's preference for less standalone functions and avoids macros.

## Implementation Steps

1. Add `ssa_name` method to `Document<'a, L>` in `document/builder.rs` or `document/ir_render.rs`.
2. Replace the 3 occurrences in `ir_render.rs` with calls to `self.ssa_name(value)`.
3. Add `print_ssa_ref` and `print_ssa_type` private helpers on Document.
4. Simplify the `ResultValue` and `SSAValue` `PrettyPrint` impls to delegate to those helpers.
5. Verify that block name resolution (`^name`) in `print_block` stays separate since it uses `BlockInfo.name` not the SSA info pattern.

## Risk Assessment

**Low risk.** Pure refactoring with no behavior change. The `ssa_name` method is additive. The `PrettyPrint` impls become thinner wrappers.

One consideration: the `GetInfo` trait and `Id: From<V>` bounds must be satisfied by `SSAValue`, `ResultValue`, `Port`, and `BlockArgument`. Need to verify all four implement the required traits. `Port` and `BlockArgument` are aliases for `SSAValue` so this should hold.

## Testing Strategy

- Existing snapshot tests in `kirin-prettyless` cover all the rendering paths. Run `cargo insta review` after changes.
- Existing roundtrip tests in `tests/roundtrip/` exercise the full parse-print cycle.
- No new test types needed -- this is a pure refactor of existing behavior.
