# Derive (Small) -- Final Report

**Crates:** kirin-derive-ir (885 lines), kirin-derive-interpreter (832 lines), kirin-derive-prettyless (129 lines)
**Total:** ~1846 lines
**Reviewers:** PL Theorist, Implementer, Compiler Engineer, Physicist

---

## Executive Summary

The three small derive crates are well-structured, clean, and consistent. They correctly encode the compositional structure of the trait system using the template architecture from `kirin-derive-toolkit`. Zero clippy suppressions. Good error diagnostics with span-preserving messages. The review surfaced one P2 finding (undocumented `#[callable]` attribute) and several minor P3 items. No P0 or P1 findings.

| Severity | Count |
|----------|-------|
| P0       | 0     |
| P1       | 0     |
| P2       | 2     |
| P3       | 5     |

---

## P2 Findings

### DS-P2-1. `#[callable]` attribute undocumented at the derive level
**Source:** Physicist DS3, Compiler Engineer DS-CC-4 (positive framing noted good validation but not docs)
**Confidence:** High
**Files:** `crates/kirin-derive-interpreter/src/lib.rs:19-35`

The `#[callable]` attribute on variants controls `SSACFGRegion` delegation and `CallSemantics` dispatch. There are no doc comments on `derive_ssa_cfg_region` or `derive_call_semantics` explaining what `#[callable]` does, when to use it, or how it interacts with `#[wraps]`. A user must reverse-engineer usage from the toy-lang example. The validation error ("requires at least one #[callable] variant") is good but insufficient for discovery.

**Action:** Add doc comments to the `SSACFGRegion` and `CallSemantics` proc-macro entry points documenting `#[callable]` semantics and interaction with `#[wraps]`.

### DS-P2-2. `#[diagnostic::on_unimplemented]` missing on interpreter traits referenced in generated bounds
**Source:** Compiler Engineer DS-CC-6
**Confidence:** Medium
**Files:** Generated where clauses reference `Interpreter<'__ir>`, `CallSemantics<'__ir, __CallSemI>`, `From<InterpreterError>`

When derive-generated bounds are unsatisfied, compiler errors reference mangled names like `__InterpI` and point into generated code. Adding `#[diagnostic::on_unimplemented]` to `Interpreter` and `CallSemantics` in `kirin-interpreter` (not in these crates) would improve the derive user experience. This reinforces Phase 1 finding **P2-E** (diagnostics on key traits).

**Action:** Address in kirin-interpreter alongside P2-E. Not actionable in these crates directly.

---

## P3 Findings

### DS-P3-1. `LocalFieldIterConfig` / `LocalPropertyConfig` duplicate toolkit types
**Source:** Implementer DS1
**Confidence:** High
**Files:** `crates/kirin-derive-ir/src/generate.rs:14-63`, `crates/kirin-derive-toolkit/src/template/trait_impl.rs:305-316`

`LocalFieldIterConfig` has identical fields to `FieldIterConfig`. The local wrapper exists only because the toolkit type is not `Copy`. Since all fields are `&'static str` + small `Copy` enums, making `FieldIterConfig` and `BoolPropertyConfig` derive `Copy` in the toolkit would eliminate ~60 lines of wrapper code and the `to_field_iter_config`/`to_bool_property_config` conversion functions.

**Action:** Add `Copy` to `FieldIterConfig` and `BoolPropertyConfig` in kirin-derive-toolkit, then remove the local wrappers.

### DS-P3-2. `parse_pretty_crate_path` duplicates attribute parsing logic
**Source:** Implementer DS4, Compiler Engineer DS-CC-5
**Confidence:** Medium
**Files:** `crates/kirin-derive-prettyless/src/generate.rs:43-64`

Manual `parse_nested_meta` for `#[pretty(crate = ...)]` duplicates the pattern in `kirin-derive-toolkit::stage::parse_ir_crate_path`. A generic "parse crate path from attribute" helper in the toolkit would serve both. Currently ~20 lines; low urgency unless more attributes are added to `RenderDispatch`.

**Action:** Consider extracting a generic crate-path parser into kirin-derive-toolkit if more derive crates need the pattern.

### DS-P3-3. `CallSemantics` Result type equalization is implicit
**Source:** PL Theorist DS2
**Confidence:** High
**Files:** `crates/kirin-derive-interpreter/src/eval_call/generate.rs:63-79`

The derive picks the first callable wrapper's `Result` type and constrains all others to match (`Result = #result_type`). This is correct for the common case (homogeneous callable enums) but the constraint is not surfaced in documentation. If a user has heterogeneous callable types, the error would come from a generated `where` clause rather than a clear derive diagnostic.

**Action:** Add a doc comment on `derive_call_semantics` noting the Result-type homogeneity requirement.

### DS-P3-4. Inconsistent error conversion: `write_errors()` vs `into_compile_error()`
**Source:** Compiler Engineer DS-CC-2
**Confidence:** High
**Files:** `crates/kirin-derive-ir/src/lib.rs:17` vs `crates/kirin-derive-ir/src/lib.rs:86`

`Dialect` derive uses `darling::Error::write_errors()` while `StageMeta` and `ParseDispatch` use `syn::Error::into_compile_error()`. This follows from different upstream return types (darling vs syn) and is functionally equivalent. Cosmetic inconsistency only.

**Action:** No action required. Note for awareness.

### DS-P3-5. `interp_crate` cloned multiple times in closures
**Source:** Implementer DS3
**Confidence:** Low
**Files:** `crates/kirin-derive-interpreter/src/interpretable.rs:23,37,69,76`

The interpreter crate path is cloned 3+ times because closures capture by move. This is standard Rust closure ergonomics and has zero runtime cost (proc-macro context). Not worth restructuring.

**Action:** No action required.

---

## Filtered Findings

The following findings from individual reports were filtered as false positives, intentional design, or not actionable:

| Finding | Source | Reason Filtered |
|---------|--------|----------------|
| All-`#[wraps]` restriction on Interpretable is too restrictive | PL Theorist DS1 | Already covered by Phase 1 P1-8 (extend derive for inner dialect enums). The current restriction is sound. |
| Dialect derive produces 19 trait impls atomically | PL Theorist DS3 | Positive observation, no issue. Individual derives also exposed. |
| RenderDispatch has no validation that variants implement PrettyPrint | PL Theorist DS4 | Downstream compile errors are sufficient; adding derive-time validation would require trait resolution unavailable in proc-macros. |
| 17 proc-macro entry points in kirin-derive-ir | Physicist DS1 | Internal artifact; users only see `#[derive(Dialect)]`. The `derive_field_iter_macro!` approach is good DX for the crate maintainer. |
| FIELD_ITER_CONFIGS array must sync with macro invocations | Implementer DS2 | Low risk; both are in the same file and the pattern is clear. |
| Default crate paths require override for in-crate tests | Physicist DS5 | Standard Rust derive practice, documented in AGENTS.md. |
| `#[wraps]` and `#[callable]` as separate attributes | All | Intentional design per AGENTS.md. |

---

## Reinforcement of Phase 1 Findings

| Phase 1 Finding | Reinforced By | Notes |
|-----------------|---------------|-------|
| P1-8: Inner dialect enums lack derive support for Interpretable | PL Theorist DS1 | The all-`#[wraps]` validation confirms the current limitation is enforced, not accidental. |
| P2-E: `#[diagnostic::on_unimplemented]` on key traits | Compiler Engineer DS-CC-6 | Generated bounds with `__`-prefixed names compound the need for better diagnostics on `Interpreter` and `CallSemantics`. |

---

## Architectural Strengths

1. **Template system is effective.** All three crates use `TraitImplTemplate` + `MethodSpec` + `Custom::separate()` cleanly. The compose/add/build pattern keeps derives declarative.
2. **Good error diagnostics.** Both `Interpretable` and `CallSemantics` derives validate preconditions and name offending variants in error messages. Span-preserving errors via `darling::Error::write_errors()` and `syn::Error::into_compile_error()`.
3. **Zero clippy suppressions.** Clean code across all three crates.
4. **Thorough snapshot testing.** Both kirin-derive-interpreter and kirin-derive-prettyless have insta snapshot tests covering positive cases, error cases, struct vs enum, and attribute combinations.
5. **Minimal dependency footprint.** All three crates depend only on `kirin-derive-toolkit`, `proc-macro2`, `quote`, `syn`. No extraneous dependencies.
