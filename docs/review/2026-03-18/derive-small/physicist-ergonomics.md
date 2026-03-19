# Derive (Small) -- Physicist (Ergonomics/DX) Review

**Crates:** kirin-derive-ir, kirin-derive-interpreter, kirin-derive-prettyless
**Lines:** ~1846

## Scenario: "I want to write interpreter semantics for my dialect"

Two paths exist: (1) manual `Interpretable` impl for leaf dialects, (2) `#[derive(Interpretable)]` for wrapper enums. The derive path is zero-lifetime, zero-boilerplate for delegation. The manual path requires understanding the `Interpretable<'ir, I>` signature with L-on-method. The toy-lang example (`language.rs:16-34`) demonstrates the ideal: `#[derive(Interpretable, SSACFGRegion)]` with `#[wraps]` and `#[callable]` just works.

## Concept Budget: Interpreter Derive

| Concept | Required? | Where learned |
|---------|-----------|---------------|
| `#[derive(Interpretable)]` | Yes | derive-interpreter |
| `#[wraps]` on each variant | Yes | dialect pattern |
| `#[callable]` on function-like variants | For CallSemantics | toy-lang |
| `#[derive(SSACFGRegion)]` | For function bodies | toy-lang |
| `Interpretable<'ir, I>` trait signature | Manual impls only | kirin-interpreter |
| `Continuation` variants | Manual impls only | kirin-interpreter |
| `interp.read/write` | Manual impls only | kirin-interpreter |

**Derive path: 4 concepts.** Manual path: 7 concepts. The derive cuts concept count nearly in half.

## Findings

### DS1. 17 proc-macro entry points in kirin-derive-ir with macro-generated boilerplate (P3, medium confidence)

`kirin-derive-ir/src/lib.rs:38-79` uses `derive_field_iter_macro!` and `derive_property_macro!` to generate 17 proc-macro functions (HasArguments, HasArgumentsMut, HasResults, etc.). From a user perspective, these are all auto-derived by `#[derive(Dialect)]` -- users never write `#[derive(HasArguments)]` directly. The 17 entry points are an internal artifact, not a DX concern. No action needed.

### DS2. Interpretable derive generates clear error messages for misuse (strength)

`kirin-derive-interpreter/src/interpretable.rs:52-66`: When a variant lacks `#[wraps]`, the derive emits a clear message naming the offending variants. The validation tests (`test_interpretable_validation_error_non_wraps`, `test_interpretable_all_non_wraps_error`) confirm good diagnostics. This is an example of good DX.

### DS3. `#[callable]` is undocumented at the derive level (P2, high confidence)

The `#[callable]` attribute on variants (used in toy-lang `language.rs:22-23`) controls SSACFGRegion delegation and CallSemantics blanket impl. There is no doc comment on the `derive_ssa_cfg_region` or `derive_call_semantics` proc-macro functions explaining what `#[callable]` does. A user tracing "I want my dialect to support function calls" would need to study the toy-lang example to discover this.

**File:** `kirin-derive-interpreter/src/lib.rs:19-35`

### DS4. kirin-derive-prettyless is minimal and clean (strength)

At 2 files and a single derive (`RenderDispatch`), this crate has zero ergonomic issues. Users interact with it only through `#[derive(RenderDispatch)]` on stage enums, and the attribute surface (`#[stage(...)]`, `#[pretty(...)]`) is shared with StageMeta.

### DS5. Default crate paths require override for in-crate tests (P3, low confidence)

`kirin-derive-interpreter/src/interpretable.rs:9-10` defaults to `::kirin_interpreter` and `::kirin::ir`. Tests inside individual crates need `#[kirin(crate = kirin_ir)]`. This is standard Rust derive practice and well-documented in AGENTS.md. No action needed beyond what exists.
