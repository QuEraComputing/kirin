# U5: Printer Review Report (kirin-prettyless)

**Date:** 2026-03-22
**Scope:** `crates/kirin-prettyless/` (~22 files) + `crates/kirin-derive-prettyless/` (2 files)
**Reviewers:** Formalism, Code Quality, Ergonomics/DX, Soundness Adversary, Dialect Author, Compiler Engineer

---

## Prior Review Items -- Status Check

| Prior Finding | Status |
|---|---|
| SSA name resolution duplicated 7x | **Fixed.** Centralized in `Document::ssa_name()` (builder.rs:135). All call sites use it. |
| `RenderBuilder::to_string()` rename to `into_string()` | **Fixed.** Method is `into_string()` at traits.rs:146. Docs updated. |
| `bon` dead dependency | **Fixed.** Not in Cargo.toml. |
| `bat` default feature pulling heavy deps | **Not fixed.** See P1 finding below. |
| `PrettyPrintViaDisplay` marker trait | **Implemented.** traits.rs:251 with blanket `PrettyPrint` impl. |

---

## Strengths

1. **Clean trait hierarchy.** `PrettyPrint` -> `PrettyPrintViaDisplay` (marker) -> `PrettyPrintExt` (blanket) is a well-layered design. The `PrettyPrintViaDisplay` marker eliminates boilerplate for Display-based types while keeping the trait coherent.

2. **Centralized SSA name resolution.** `Document::ssa_name()` (builder.rs:135-147) is the single source of truth for resolving SSA value names. The prior 7x duplication is fully fixed.

3. **Builder pattern with `#[must_use]`.** `RenderBuilder` (traits.rs:121) has a descriptive `#[must_use]` annotation guiding users to `.into_string()`, `.print()`, or `.bat()`.

4. **Comprehensive error handling.** `RenderError` (error.rs) properly implements `Display`, `Error` with `source()`, and `From` conversions for both `io::Error` and `fmt::Error`.

5. **Good test coverage.** Edge cases for floats (negative zero, tiny fractions), strings (quotes, newlines), empty containers, config extremes (zero width, zero tab spaces), and error variants are all tested. Graph printing (digraph/ungraph) has thorough snapshot tests.

6. **`RenderDispatch` derive is minimal and correct.** The proc macro in `kirin-derive-prettyless` generates a simple match-arm dispatch with proper crate-path customization.

7. **Interior mutability for function context** is well-motivated. The `Cell<Option<GlobalSymbol>>` for `function_name` (builder.rs:32) is documented with a clear justification for why `Cell` is needed given the `&'a self` borrow pattern.

---

## High Priority

### [P1] [confirmed] bat dependency pulls default features including `application`, `git`, `bugreport`

**File:** `crates/kirin-prettyless/Cargo.toml:7`
**Perspective:** Compiler Engineer

The bat dependency is specified as `bat = { version = "0.26", optional = true }` without `default-features = false`. When the `bat` feature is enabled, this pulls in bat's `application`, `bugreport`, `git`, and other heavy features -- adding ~313 transitive dependencies on top of the base ~140. The crate only uses `PrettyPrinter`, `PagingMode`, and syntax highlighting, none of which require the `application` or `git` features.

**Suggested action:** Add `default-features = false` and enable only the features actually needed (likely just `regex-onig` or `regex-fancy` for syntax highlighting):
```toml
bat = { version = "0.26", optional = true, default-features = false }
```

### [P1] [confirmed] `bat::print_str` panics on I/O error via `.unwrap()`

**File:** `crates/kirin-prettyless/src/bat.rs:14`
**Perspective:** Soundness Adversary

`print_str` calls `.print().unwrap()` on the bat `PrettyPrinter`. If stdout is a broken pipe (common when piping to `head` or `less` that exits early), or if the terminal is unavailable, this will panic. The callers (`FunctionRenderBuilder::bat` at pipeline.rs:158 and `Document::pager` at bat.rs:26) already return `Result`, so propagating the error is straightforward.

**Suggested action:** Change `print_str` to return `Result<(), io::Error>` (bat's `print()` returns `Result<bool>`), and propagate through callers. Alternatively, convert to `RenderError::Io`.

### [P1] [confirmed] `print_ports` has an inline closure duplicating `print_port_list` method

**File:** `crates/kirin-prettyless/src/document/ir_render.rs:164-175`
**Perspective:** Code Quality

The inline closure `print_port_list` inside `print_ports()` (lines 164-175) is functionally identical to the `print_port_list` method (lines 522-533). The method was presumably added later for the `_only` projection variants but the original `print_ports` was never updated to use it.

**Suggested action:** Refactor `print_ports` to call `self.print_port_list()` instead of the inline closure:
```rust
let edge_doc = self.print_port_list(edge_ports).enclose("(", ")");
```

---

## Medium Priority

### [P2] [confirmed] Name resolution pattern for `^name` entities is duplicated 3x

**File:** `crates/kirin-prettyless/src/document/ir_render.rs:97-105, 196-204, 218-226`
**Perspective:** Code Quality

The pattern for resolving block/graph names via the symbol table:
```rust
info.name()
    .and_then(|name_sym| self.stage.symbol_table().resolve(name_sym).map(|s| format!("^{}", s)))
    .unwrap_or_else(|| format!("{}", entity))
```
appears in `print_block` (line 97), `print_digraph` (line 196), and `print_ungraph` (line 218). All three follow identical logic.

**Suggested action:** Extract a helper method like:
```rust
fn resolve_caret_name<E: std::fmt::Display>(&self, name: Option<Symbol>, fallback: &E) -> String
```

### [P2] [confirmed] `%name: Type` formatting duplicated 4x without using `print_port_list`

**File:** `crates/kirin-prettyless/src/document/ir_render.rs:118, 172, 516, 530`
**Perspective:** Code Quality

The pattern `format!("%{}: {}", self.ssa_name(val), val.expect_info(self.stage).ty())` appears in block argument printing (line 118), inline port printing (line 172), `print_block_args_only` (line 516), and `print_port_list` (line 530). The block argument loop in `print_block` (lines 112-119) duplicates what `print_block_args_only` does.

**Suggested action:** Extract a `print_typed_ssa_binding` helper and refactor `print_block` to reuse `print_block_args_only`.

### [P2] [confirmed] `FunctionRenderBuilder` and `PipelineRenderBuilder` missing `#[must_use]`

**File:** `crates/kirin-prettyless/src/pipeline.rs:123, 164`
**Perspective:** Ergonomics/DX

`RenderBuilder` in traits.rs has `#[must_use]` but the pipeline builder types (`FunctionRenderBuilder`, `PipelineRenderBuilder`) do not. A user who writes `func.render(&pipeline);` (without consuming the builder) gets no warning.

**Suggested action:** Add `#[must_use = "call .into_string(), .print(), or .bat() to produce output"]` to both types.

### [P2] [confirmed] Float `PrettyPrint` does not handle NaN or infinity

**File:** `crates/kirin-prettyless/src/impls.rs:196`
**Perspective:** Soundness Adversary

The float impl uses `self.fract() == 0.0` to decide formatting. For `NaN`, `fract()` returns `NaN` and `NaN == 0.0` is false, so it falls through to `self.to_string()` which produces `"NaN"`. For positive/negative infinity, `fract()` also returns `NaN`. While this doesn't crash, the output (`"NaN"`, `"inf"`, `"-inf"`) may not roundtrip through the parser. If the IR can contain these values, they need explicit formatting/parsing support.

**Suggested action:** Add explicit branches for `is_nan()` and `is_infinite()` with documented formatting choices. Add test cases for NaN and infinity to confirm roundtrip behavior or document the limitation.

### [P2] [confirmed] `Config` fields are `pub` -- allows direct mutation bypassing builder

**File:** `crates/kirin-prettyless/src/config.rs:9-11`
**Perspective:** Formalism

`Config::tab_spaces` and `Config::max_width` are `pub` fields while also having builder methods (`with_width`, `with_tab_spaces`). This dual API is not harmful but is inconsistent -- either commit to builder-only (make fields private) or drop the builder methods. For a config struct this simple, public fields are fine, but the builder methods are then redundant ceremony.

**Suggested action:** This is a style choice. If the builder methods stay, consider making fields `pub(crate)` to enforce the builder API for external users. Or keep as-is and accept the dual API.

### [P2] [likely] `PipelineDocument` could benefit from `#[must_use]`

**File:** `crates/kirin-prettyless/src/pipeline.rs:83`
**Perspective:** Ergonomics/DX

`PipelineDocument` is a public type constructed via `PipelineDocument::new()` but only useful when `render_function()` is called. Unlike the builder types, forgetting to call `render_function()` silently discards the pipeline document.

**Suggested action:** Add `#[must_use]` to `PipelineDocument`.

---

## Low Priority

### [P3] [confirmed] Unused import: `Dialect` in `bat.rs`

**File:** `crates/kirin-prettyless/src/bat.rs:3`
**Perspective:** Code Quality

`use kirin_ir::Dialect;` is imported but `Dialect` is used only as a trait bound in the `impl` block where `L: Dialect + PrettyPrint`. This import is necessary for the trait bound, so it is not truly unused -- but worth confirming the compiler agrees (it may elide the warning because of the impl block usage).

**Suggested action:** Verify with `cargo check --features bat`. No action needed if clean.

### [P3] [confirmed] `Document::list` allocates a separator clone per iteration

**File:** `crates/kirin-prettyless/src/document/builder.rs:112-128`
**Perspective:** Formalism

`list()` takes `U: Clone + Into<Cow<'a, str>>` and clones the separator on every iteration. For typical usage (small separators like `", "`), this is negligible. However, the `Cow` bound is not leveraged -- the separator is always cloned and converted, never borrowed.

**Suggested action:** Consider accepting `&str` directly for the separator, which is the only type used in practice. This simplifies the signature without losing functionality.

### [P3] [confirmed] `strip_trailing_whitespace` returns `"\n"` for empty input

**File:** `crates/kirin-prettyless/src/document/builder.rs:174-176`
**Perspective:** Formalism

When given empty input, `strip_trailing_whitespace` returns `"\n"` rather than an empty string. This is intentional (tested in document/tests.rs:5-6) but may surprise callers who expect empty in / empty out. The behavior is consistent with the "always end with newline" convention, but worth documenting in the function's doc comment.

**Suggested action:** Add a brief doc comment noting the empty-input behavior.

### [P3] [confirmed] `RenderDispatch::render_staged_function` returns `Result<Option<String>, std::fmt::Error>` instead of `RenderError`

**File:** `crates/kirin-prettyless/src/pipeline.rs:51`
**Perspective:** Formalism

The trait method returns `fmt::Error` while the rest of the API uses `RenderError`. The blanket impl on `StageInfo<L>` (line 74) calls `doc.render()` which returns `fmt::Error`, so this is the natural error type. However, callers in `PipelineDocument::render_function` (line 110) use `?` which auto-converts via `From<fmt::Error> for RenderError`. The asymmetry means implementors of `RenderDispatch` get `fmt::Error` while consumers get `RenderError`.

**Suggested action:** Consider changing the trait's error type to `RenderError` for consistency with the public API. This is a minor breaking change to the trait.

### [P3] [confirmed] No `PrettyPrint` impl for `&str` or borrowed types

**File:** `crates/kirin-prettyless/src/impls.rs`
**Perspective:** Ergonomics/DX

`PrettyPrint` is implemented for `String` but not `&str`. While dialect authors typically work with owned types in IR structs, test code or ad-hoc printing sometimes wants to render string slices. The blanket `impl<T: PrettyPrint> PrettyPrint for &T` would cover this if it existed (it doesn't due to coherence with `PrettyPrintViaDisplay`).

**Suggested action:** Consider adding `impl PrettyPrintViaDisplay for &str {}` or a targeted `impl PrettyPrint for &str`. Low priority since IR types are owned.

---

## Filtered Findings (intentional design, not flagged)

- **`PrettyPrintViaDisplay` existence** -- confirmed implemented, working as intended.
- **`into_string()` naming** -- confirmed renamed from `to_string()`.
- **`bon` dependency** -- confirmed removed.
- **`Cell` for `function_name`** -- interior mutability is documented and justified by the `&'a self` borrowing pattern.
- **`Deref<Target = Arena>` on `Document`** -- standard pattern for arena-based builders, enables direct `doc.text()` calls.
- **Inline `PrettyPrint` impl for `SimpleLanguage` in tests** -- appropriate for test-only code, avoids circular deps.
