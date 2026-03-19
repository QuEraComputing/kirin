# kirin-derive-chumsky — Final Review Report

## High Priority (P0-P1)

### P1-1: Direct `darling` dependency violates workspace convention
**Source:** Compiler Engineer
**Verified:** Yes. `crates/kirin-derive-chumsky/Cargo.toml:13` lists `darling.workspace = true` as a direct dependency. `src/attrs.rs:3` imports `darling::{FromDeriveInput, FromField, FromVariant}` directly. Neither `kirin-derive-ir` nor `kirin-derive-interpreter` have a direct `darling` dependency — they use the toolkit re-export as required by AGENTS.md. The `#[derive(FromDeriveInput)]` etc. macros require darling as a direct proc-macro dependency for the derive attributes to resolve, so the fix may require the toolkit to re-export the darling derive macros or an explicit exception in AGENTS.md.
**Files:** `crates/kirin-derive-chumsky/Cargo.toml:13`, `crates/kirin-derive-chumsky/src/attrs.rs:3`
**Action:** Investigate whether `kirin-derive-toolkit` can re-export darling derive macros (it may already via `prelude::darling`). If so, switch `attrs.rs` imports to `kirin_derive_toolkit::prelude::darling::{FromDeriveInput, FromField, FromVariant}` and remove the direct dependency. If darling's proc-macro derives require the crate in `Cargo.toml`, document this as a known exception.

### P1-2: Unused `_has_dialect_parser_bounds` parameter
**Source:** Implementer
**Verified:** Yes. `src/codegen/ast/trait_impls.rs:177` takes `_has_dialect_parser_bounds: &[TokenStream]` but never reads it. The function recomputes its own `has_dialect_parser_base_bounds` on line 186-189. Removing this parameter also eliminates the `#[allow(clippy::too_many_arguments)]` on line 168.
**Files:** `crates/kirin-derive-chumsky/src/codegen/ast/trait_impls.rs:168-177`
**Action:** Remove the parameter and update all call sites. Simple cleanup.

## Medium Priority (P2)

### P2-1: `is_missing_type_error` relies on string matching
**Source:** Compiler Engineer
**Verified:** Yes. `src/input.rs:37-39` checks `message.contains("Missing field \`type\`")`. If darling changes its error message wording, this breaks silently (the fallback path would stop triggering, causing confusing errors for users of types without `#[kirin(type = ...)]`).
**Files:** `crates/kirin-derive-chumsky/src/input.rs:37-40`
**Action:** Consider parsing with an `Option<syn::Path>` for the `type` field and checking presence programmatically, or pin this to a specific darling behavior with a comment and a test that verifies the expected error message.

### P2-2: Format string DSL lacks standalone documentation
**Source:** Physicist
**Verified:** Yes. The format string syntax (`{field}`, `{field:name}`, `{field:type}`, `{.keyword}`, escaped braces, literal tokens) is only discoverable by reading existing dialect code or the `format.rs` source. No reference document exists.
**Action:** Create a format string reference, either as a doc comment on `format.rs` public items or as a section in the existing docs. This is the primary user-facing API of the crate and should be documented.

### P2-3: Bounds collection logic is repeated across trait impl generators
**Source:** Implementer
**Verified:** Likely. The manual Clone/Debug/PartialEq impl generators each collect wrapper types and build bound lists with similar patterns.
**Files:** `crates/kirin-derive-chumsky/src/codegen/ast/trait_impls.rs`
**Action:** Extract a shared `BoundsCollector` or helper function. Low urgency but would reduce maintenance surface if the bounds logic needs to change.

## Low Priority (P3)

### P3-1: `.or()` ordering creates implicit parsing priority
**Source:** PL Theorist
Enum variant order determines parse priority via left-to-right `.or()` chains. This is standard PEG semantics and matches chumsky's design. No ambiguity detection exists, but in practice MLIR-style operations have non-overlapping keyword prefixes (namespace + op name), making this a theoretical rather than practical concern. No action needed unless users report ambiguity issues.

### P3-2: Large enum variant count untested
**Source:** Compiler Engineer (modified)
The generated code uses `.or()` chains, not `choice()` tuples, so the compiler engineer's concern about chumsky's `choice()` arity limit is a false positive. However, very long `.or()` chains (50+ variants) could stress the compiler's type inference. Low risk since dialect enums are typically small.

### P3-3: Namespace composition semantics for deeply nested wrappers
**Source:** PL Theorist
Namespace prefixes compose additively. Ambiguity from different nesting paths producing identical namespace arrays is theoretically possible but unlikely in practice given that dialect names are unique.

### P3-4: Generated code traceability
**Source:** Physicist
`#[derive(HasParser)]` silently generates 4 items (AST type, HasDialectParser, EmitIR, ParseEmit). Errors in generated code can be hard to trace. This is inherent to proc-macro code generation. The existing validation pass catches most user errors at the format string level before codegen runs.

## Strengths

1. **Shared format string for parser and printer** — Single source of truth ensures roundtrip correctness by construction. The parse-print duality is clean and well-implemented.

2. **Format string DSL is well-designed** — Reads like the target syntax. `{result:name} = {.add} {lhs}, {rhs} -> {result:type}` is immediately understandable. Low concept count for users.

3. **HRTB elimination via ParseDispatch** — Replacing the HRTB-based dispatch with a monomorphic `ParseDispatch` trait is a significant ergonomic win. Users never encounter `for<'t>` bounds.

4. **Thorough format string validation** — Catches missing fields, invalid projections (`:name`/`:type` on wrong field types), and reports all errors at once rather than one-at-a-time.

5. **Clean three-phase codegen** — AST generation, parser generation, and emit generation are independent passes with clear separation of concerns. The 22-file `codegen/` directory is well-organized into `ast/`, `parser/`, `pretty_print/`, `emit_ir/` subdirectories.

6. **Good generated code hygiene** — Fully qualified paths, proper lifetime handling, `BoxedParser` for type erasure.

## Filtered Findings

| Finding | Source | Reason Filtered |
|---------|--------|----------------|
| Format string DSL is intentional | PL Theorist | Listed as design context — not a finding |
| `#[derive(HasParser)]` generates ParseEmit automatically | Physicist | Intentional per design context |
| `chumsky` direct dependency adds compile time | Compiler Engineer | **False positive**: `chumsky` is used at proc-macro time to parse format strings (`format.rs:16-18`), not just for generated code paths. The dependency is necessary. |
| `choice()` arity limit for large enums | Compiler Engineer | **False positive**: Generated code uses `.or()` chains, not `choice()` tuples |
| AST codegen repetition across passes | Implementer | Inherent to the problem domain; each pass produces different output from the same input |
| Missing `#[must_use]` on internal functions | Implementer | Not meaningful for proc-macro codegen internals |
| Format string constrained to Kirin lexer token vocabulary | PL Theorist | Intentional self-bootstrapping design |
| Bidirectional transformation is restricted | PL Theorist | Informational; the restriction (one syntax per op) is acceptable for IR text formats |

## Suggested Follow-Up Actions

1. **Immediate (P1):** Remove unused `_has_dialect_parser_bounds` parameter — straightforward cleanup, no design decisions needed.

2. **Soon (P1):** Resolve the direct `darling` dependency. Check if `use kirin_derive_toolkit::prelude::darling::{FromDeriveInput, FromField, FromVariant}` works for both imports and `#[derive()]` resolution. If not, document the exception.

3. **Next sprint (P2):** Harden `is_missing_type_error` against darling version changes. Add a regression test that verifies the expected error message format.

4. **Next sprint (P2):** Write format string DSL reference documentation. This is the primary user-facing API and currently undocumented beyond source code.

5. **Backlog (P2):** Extract shared bounds collection into a helper to reduce duplication in `trait_impls.rs`.
