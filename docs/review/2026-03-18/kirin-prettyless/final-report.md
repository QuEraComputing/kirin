# kirin-prettyless — Final Review Report

## High Priority (P0-P1)

### P1: `bat` is in default features for a library crate
**Source:** Compiler Engineer
**File:** `crates/kirin-prettyless/Cargo.toml:16`
**Verified:** Yes. `default = ["serde", "bat"]` means any downstream library crate that depends on `kirin-prettyless` compiles `bat` + `syntect` + `regex` and all transitive dependencies unless it explicitly opts out with `default-features = false`. For a library crate, syntax highlighting should be opt-in, not opt-out. End-user binaries (like `toy-lang`) should enable `bat`; the library default should not include it.
**Action:** Change default features to `default = ["serde"]`. Update downstream `Cargo.toml` entries that need `bat` to explicitly enable it.

### P1: SSA name resolution duplicated across `impls.rs` and `ir_render.rs`
**Source:** Implementer
**Files:** `src/impls.rs:11-101`, `src/document/ir_render.rs:52-60, 114-122, 178-186`
**Verified:** Yes. The pattern "look up name symbol in symbol table, fall back to raw ID" appears 6+ times with minor variations. The `ResultValue` and `SSAValue` `PrettyPrint` impls in `impls.rs` are nearly identical across all three methods (`namespaced_pretty_print`, `pretty_print_name`, `pretty_print_type`).
**Action:** Extract a `resolve_ssa_name(&self, info: &Item<SSAInfo<L>>, id: impl Into<Id>) -> String` helper on `Document`. Then consolidate `ResultValue`/`SSAValue` impls to use it. Consider a shared trait or generic impl since the two types differ only in how they obtain `&Item<SSAInfo<L>>`.

## Medium Priority (P2)

### P2: `bon` is a dead dependency
**Source:** Compiler Engineer (noted as "used for builder patterns")
**File:** `crates/kirin-prettyless/Cargo.toml:8`
**Verified:** `bon` is listed as a dependency but **not imported or used anywhere** in `crates/kirin-prettyless/src/`. The `RenderBuilder`, `FunctionRenderBuilder`, and `PipelineRenderBuilder` are all hand-written. This is pure dead weight adding proc-macro compile time.
**Action:** Remove `bon` from `[dependencies]` in `Cargo.toml`.

### P2: `RenderDispatch` returns `std::fmt::Error` (no diagnostic info)
**Source:** PL Theorist, Compiler Engineer
**File:** `src/pipeline.rs:46-51`
**Verified:** Yes. `RenderDispatch::render_staged_function` returns `Result<Option<String>, std::fmt::Error>`, while consumers like `PipelineDocument::render_function` use `RenderError`. `std::fmt::Error` is a unit type with zero diagnostic information. Any rendering failure in a staged function is opaque to the caller.
**Action:** Change `RenderDispatch` return type to `Result<Option<String>, RenderError>` (or at minimum `Box<dyn Error>`). This is a breaking change to a derive-generated trait, so coordinate with `kirin-derive-prettyless`.

### P2: f32 and f64 PrettyPrint impls are identical
**Source:** Implementer
**File:** `src/impls.rs:236-270`
**Verified:** Yes. Both have the exact same body (`if self.fract() == 0.0 { format!("{:.1}", self) } else { self.to_string() }`). The existing `impl_pretty_print_int!` macro pattern (used for integer types) could be extended, or a separate `impl_pretty_print_float!` macro could handle both.
**Action:** Unify with a macro.

### P2: `PrettyPrint` for leaf types duplicates `Display`
**Source:** Physicist
**Files:** `src/impls.rs` (integers, bool, String), various dialect type impls
**Verified:** Yes. Most leaf-type `PrettyPrint` impls are just `doc.text(self.to_string())`. The trait signature requires `L`, `namespace`, and `where L::Type: Display` that are irrelevant to these impls.
**Action:** Consider a `PrettyPrintViaDisplay` marker trait with a blanket `PrettyPrint` impl, or a simple derive macro for the `doc.text(self.to_string())` case. A blanket `PrettyPrint for T: Display` would conflict with structured types, so a marker trait is the right approach.

### P2: Missing `#[must_use]` annotations
**Source:** Implementer
**Verified:** Yes. No `#[must_use]` on `RenderBuilder::to_string()`, `Document::new()`, `PrettyPrintExt::sprint()`, or `PrettyPrintExt::render()`.
**Action:** Add `#[must_use]` to builder constructors and methods that return values.

## Low Priority (P3)

### P3: `sprint` panics on render failure
**Source:** PL Theorist, Implementer, (also present in `PrintExt::sprint` and `PipelinePrintExt::sprint`)
**Files:** `src/traits.rs:214-215`, `src/pipeline.rs:221,247`
**Verified:** Yes. Three `sprint` methods use `.expect("render failed")` with a generic panic message. This is a deliberate convenience API tradeoff (the builder `.to_string()` returns `Result`), but the panic message should at least include the error.
**Action:** Improve panic messages to include the error, e.g., `.unwrap_or_else(|e| panic!("render failed: {e}"))`. Do not change `sprint` to return `Result` -- the `render().to_string()` path already serves that purpose.

### P3: `RenderBuilder::to_string` shadows std naming convention
**Source:** Implementer
**File:** `src/traits.rs:129`
**Verified:** Yes. `RenderBuilder::to_string(self)` takes ownership and returns `Result<String, RenderError>`, which has a different signature from `Display::to_string(&self) -> String`. Since `RenderBuilder` does not implement `Display`, this is not a functional issue, but `into_string()` or `render_to_string()` would be clearer.
**Action:** Low priority. Consider renaming in a future API revision, but not urgent since the builder is always used via method chaining.

### P3: Graph name resolution pattern duplicated
**Source:** Implementer
**File:** `src/document/ir_render.rs:147-155, 200-208`
**Verified:** Yes. `print_digraph` and `print_ungraph` have identical name resolution code. This is a subset of the broader SSA name resolution duplication and would be resolved by the same helper method.
**Action:** Addressed by the P1 name resolution extraction.

### P3: `pretty_print_name`/`pretty_print_type` defaults may be misleading
**Source:** PL Theorist
**File:** `src/traits.rs:58-79`
**Verified:** Yes. Both default to `pretty_print`, which is correct for leaf types but wrong if a type appears in `{field:name}` or `{field:type}` format positions without overriding. The derive macro generates the overrides, so this only affects manual impls.
**Action:** Document this contract more explicitly in the trait docs.

## Strengths

1. **Clean layered API:** The three-tier design (`sprint` shorthand, `render` builder, `Document` internals) provides the right abstraction at each level. `pipeline.sprint()` is a single call; full control is available via builders. This was noted by all four reviewers.

2. **Wadler-Lindig foundation:** Using `prettyless` (arena-allocated Wadler-Lindig) is the standard choice for compiler IR pretty printing. Output quality is optimal, memory behavior is good (arena allocation), and the algebraic document model composes well.

3. **Roundtrip property:** The `parse(sprint(ir)) == ir` invariant is maintained by sharing format strings between `#[derive(HasParser)]` and `#[derive(PrettyPrint)]`. This is a strong correctness guarantee.

4. **Zero clippy workarounds:** No `#[allow(...)]` in the crate. Clean from a lint perspective.

5. **`RenderDispatch` design:** Monomorphic dispatch with blanket impl for `StageInfo<L>` and `#[derive(RenderDispatch)]` for stage enums is the right pattern -- it mirrors `ParseDispatch` in the parser layer and avoids trait objects.

6. **`PipelinePrintExt` hides all complexity:** End users call `pipeline.sprint()` and get the full multi-stage output. The concept budget for the common case is minimal (2 concepts for pipeline printing, 4 for single-dialect printing).

## Filtered Findings

| Finding | Source | Reason Filtered |
|---------|--------|----------------|
| `PrettyPrint` monomorphizes O(M*N) | Compiler Engineer | Informational, inherent to the design. M (node types using `sprint`) is small in practice. No actionable change. |
| `PrettyPrint` trait exposes `L`/`namespace` for leaf impls | Physicist | By design. The trait must be generic over `L` for recursive block/region rendering. Leaf types just ignore these params. Addressed by the `PrettyPrintViaDisplay` suggestion (P2). |
| `kirin-chumsky` depends on `kirin-prettyless` creating coupling | Compiler Engineer | By design. The parser and printer share format strings for roundtrip correctness. This coupling is intentional. |
| `petgraph` is a direct dependency | Compiler Engineer | Required for graph rendering (`DiGraph`/`UnGraph`). Same version as `kirin-ir`. Not actionable. |
| `PrettyPrint` document construction panics on semantic errors | Compiler Engineer | The `expect_info()` calls panic on missing IR info, which is consistent with the rest of the IR API. These represent programmer errors (malformed IR), not user-facing errors. Not a change candidate. |

## Suggested Follow-Up Actions

1. **Immediate (low effort):**
   - Remove `bon` from `Cargo.toml` (dead dependency)
   - Unify `f32`/`f64` impls with a macro
   - Improve `sprint` panic messages to include the error

2. **Short-term (moderate effort):**
   - Move `bat` out of default features
   - Extract SSA name resolution helper on `Document`
   - Consolidate `ResultValue`/`SSAValue` impls
   - Add `#[must_use]` annotations

3. **Medium-term (design work):**
   - Change `RenderDispatch` return type from `std::fmt::Error` to `RenderError`
   - Introduce `PrettyPrintViaDisplay` marker trait for leaf types
   - Document `pretty_print_name`/`pretty_print_type` override contract
