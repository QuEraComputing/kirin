# Implementer -- Code Quality Review: kirin-prettyless

## Clippy Workaround Audit

No `#[allow(...)]` attributes found in any source files under `crates/kirin-prettyless/src/`. This crate is completely clean from a clippy workaround perspective.

## Logic Duplication

### 1. SSA name resolution pattern repeated 5+ times (P1, confirmed)

**File:** `src/document/ir_render.rs`

The following pattern appears at lines 52-60, 114-122, and 178-186:
```rust
let name = if let Some(name_sym) = info.name() {
    self.stage.symbol_table().resolve(name_sym).cloned()
        .unwrap_or_else(|| format!("{}", Id::from(*x).raw()))
} else {
    format!("{}", Id::from(*x).raw())
};
```

**Also in `src/impls.rs`**, the SSAValue and ResultValue `PrettyPrint` impls at lines 11-55 and 57-101 repeat the same name-resolution-and-fallback logic for `pretty_print`, `pretty_print_name`, and `namespaced_pretty_print`.

**Suggestion:** Extract a `resolve_ssa_name(&self, info: &Item<SSAInfo<L>>, id: impl Into<Id>) -> String` method on `Document` that encapsulates the name-resolution-with-fallback pattern. This would reduce 6+ occurrences to single method calls.

### 2. ResultValue and SSAValue PrettyPrint impls are nearly identical (P1, confirmed)

**File:** `src/impls.rs:11-101`

The `PrettyPrint` impls for `ResultValue` (lines 11-55) and `SSAValue` (lines 57-101) have identical logic in all three methods (`namespaced_pretty_print`, `pretty_print_name`, `pretty_print_type`). The only difference is the type being `.expect_info()`-ed.

Both resolve to `&Item<SSAInfo<L>>` via `GetInfo`, then use the same name-lookup-and-format pattern. A shared helper or a blanket impl over a common trait could eliminate this duplication.

### 3. f32 and f64 PrettyPrint impls are identical (P2, confirmed)

**File:** `src/impls.rs:236-270`

The `f32` and `f64` impls have identical bodies (`if self.fract() == 0.0 { format!("{:.1}", self) } else { self.to_string() }`). A macro or generic impl could unify them.

### 4. Graph name resolution pattern (P3, confirmed)

**File:** `src/document/ir_render.rs:147-155` and `src/document/ir_render.rs:200-208`

DiGraph and UnGraph name resolution follows the same pattern:
```rust
let graph_name = info.name()
    .and_then(|name_sym| self.stage.symbol_table().resolve(name_sym).map(|s| format!("^{}", s)))
    .unwrap_or_else(|| format!("{}", graph_id));
```

This also appears in `print_block` (lines 32-40). Extract as `resolve_named_node(&self, name: Option<Symbol>, fallback: impl Display) -> String`.

## Rust Best Practices

### Missing `#[must_use]` annotations (P2, confirmed)

Zero `#[must_use]` in the crate. Key candidates:
- `RenderBuilder::to_string()` -- returns `Result<String>`
- `Document::new()` -- constructor
- `PrettyPrintExt::sprint()` -- returns `String`
- `PrettyPrintExt::render()` -- returns `RenderBuilder`

### `sprint` panics on render failure (P2, confirmed)

**File:** `src/traits.rs:214-216`

```rust
fn sprint(&self, stage: &StageInfo<L>) -> String {
    self.render(stage).to_string().expect("render failed")
}
```

The `sprint` method uses `.expect()` which will panic on render errors. This is a convenience method, but the panic message is generic. Consider either:
1. Returning `Result<String, RenderError>` (breaking change)
2. Using a more descriptive panic message that includes the error

### `RenderBuilder::to_string` shadows std trait (P3, confirmed)

**File:** `src/traits.rs:129`

`RenderBuilder::to_string(self)` takes ownership and returns `Result<String, RenderError>`. This shadows the `Display::to_string(&self) -> String` method. While `RenderBuilder` does not implement `Display`, the naming could cause confusion. Consider renaming to `render_to_string()` or `into_string()`.

## Summary

- P1 confirmed -- `src/impls.rs:11-101`: ResultValue and SSAValue PrettyPrint impls are copy-pasted; extract shared helper
- P1 confirmed -- `src/document/ir_render.rs`: SSA name resolution pattern duplicated 5+ times; extract method
- P2 confirmed -- `src/impls.rs:236-270`: f32/f64 PrettyPrint impls identical; unify with macro
- P2 confirmed -- Missing `#[must_use]` across the crate
- P2 confirmed -- `src/traits.rs:215`: `sprint` panics with generic message on render failure
- P3 confirmed -- Graph name resolution duplicated in ir_render.rs
- P3 confirmed -- `RenderBuilder::to_string` shadows std naming convention
