# Multi-Result Values — Plan Index

**Date:** 2026-03-22
**Design document:** `docs/design/multi-result-values.md`
**Review report:** `docs/review/2026-03-22-resultvalue-paths/report.md`
**Pattern:** in-place
**Total work items addressed:** 8 of 11 (W9 eliminated — no changes needed; W11 absorbed into each wave)

---

## Dependency Graph

```
wave-0 (parallel: builder-template, format-dsl)
       |
    wave-1 (single agent: continuation + exec + eval_block)
       |
    wave-2 (parallel: scf-dialect, function-dialect)
       |
    wave-3 (single agent: unpack-crate)
```

## Wave 0

**Depends on:** nothing (prerequisites)

| Plan File | Title | Work Item(s) | Agent Role | Crate(s) |
|-----------|-------|--------------|------------|----------|
| `wave-0/builder-template-plan.md` | Builder template — lift Vec/Option ResultValue rejection | W1 | Implementer | kirin-derive-toolkit |
| `wave-0/format-dsl-plan.md` | Text format DSL — `[...]` optional section syntax | W2 | Implementer | kirin-lexer, kirin-derive-chumsky |

## Wave 1

**Depends on:** Wave 0 complete and merged. (Wave 1 does not directly use Vec<ResultValue> or `[...]` in its own code, but downstream compat fixes in kirin-scf/kirin-function need clean compilation.)

| Plan File | Title | Work Item(s) | Agent Role | Crate(s) |
|-----------|-------|--------------|------------|----------|
| `wave-1/continuation-and-exec-plan.md` | Continuation enum + run_nested_calls + eval_block | W3, W4, W5, W6 | Implementer | kirin-interpreter |

Note: W6 (abstract interpreter changes) is included in the Wave 1 plan because it touches the same files (`result.rs`, `fixpoint.rs`, `interp.rs`) as W3-W5 and must be done atomically with the Continuation enum change. Separating it would require the Continuation change to land with an inconsistent abstract interpreter.

## Wave 2

**Depends on:** Wave 0 + Wave 1 complete and merged.

| Plan File | Title | Work Item(s) | Agent Role | Crate(s) |
|-----------|-------|--------------|------------|----------|
| `wave-2/scf-dialect-plan.md` | SCF dialect multi-result changes | W7 | Implementer | kirin-scf |
| `wave-2/function-dialect-plan.md` | Function dialect multi-result changes | W8 | Implementer | kirin-function |

## Wave 3

**Depends on:** Wave 1 complete and merged. (Can technically start after Wave 1, parallel with Wave 2, but sequenced after for simplicity.)

| Plan File | Title | Work Item(s) | Agent Role | Crate(s) |
|-----------|-------|--------------|------------|----------|
| `wave-3/unpack-crate-plan.md` | kirin-unpack new crate | W10 | Builder | kirin-unpack (new) |

## Agent Assignments

| Agent Name | Role | Wave | Plan File | Files Touched |
|------------|------|------|-----------|---------------|
| builder-template | Implementer | 0 | `wave-0/builder-template-plan.md` | kirin-derive-toolkit: helpers.rs, collection.rs |
| format-dsl | Implementer | 0 | `wave-0/format-dsl-plan.md` | kirin-lexer: lib.rs (EscapedLBracket/EscapedRBracket tokens); kirin-derive-chumsky: format.rs, validation.rs, codegen/parser/, codegen/pretty_print/, visitor.rs |
| continuation-exec | Implementer | 1 | `wave-1/continuation-and-exec-plan.md` | kirin-interpreter: control.rs, call.rs, stack/exec.rs, stack/frame.rs, stack/transition.rs, stack/stage.rs, stack/call.rs, stack/dispatch.rs, abstract_interp/*, result.rs, block_eval.rs, tests/*; kirin-scf (mechanical compat); kirin-function (mechanical compat); example/toy-lang (mechanical compat) |
| scf-dialect | Implementer | 2 | `wave-2/scf-dialect-plan.md` | kirin-scf: lib.rs, interpret_impl.rs, tests.rs; tests/roundtrip/scf.rs |
| function-dialect | Implementer | 2 | `wave-2/function-dialect-plan.md` | kirin-function: call.rs, ret.rs, interpret_impl.rs, lib.rs; tests/roundtrip/function.rs |
| unpack-crate | Builder | 3 | `wave-3/unpack-crate-plan.md` | kirin-unpack/ (new), tests/roundtrip/unpack.rs (new) |

**File disjointness check:**
- **Wave 0:** builder-template (kirin-derive-toolkit) and format-dsl (kirin-lexer, kirin-derive-chumsky) are fully disjoint. Safe for parallel execution.
- **Wave 1:** Single agent, no disjointness concerns. Touches kirin-scf and kirin-function for mechanical compat only (these will be overwritten by Wave 2).
- **Wave 2:** scf-dialect (kirin-scf) and function-dialect (kirin-function) are fully disjoint. Safe for parallel execution. Both depend on Wave 1's mechanical compat changes being present.
- **Wave 3:** New crate, no overlaps.
- **Cross-wave sequencing:** Wave 1 writes minimal compat fixes to kirin-scf/kirin-function files. Wave 2 overwrites those same files with full multi-result support. This is safe because Wave 2 depends on Wave 1 completing first.

## Verification Checkpoints

After each wave:
1. `cargo build --workspace`
2. `cargo nextest run --workspace`
3. `cargo test --doc --workspace`
4. `cargo insta test --workspace` (if snapshots exist)

## Eliminated Work Items

| Work Item | Reason |
|-----------|--------|
| W9 (mechanical dialect updates) | Eliminated — arith, bitwise, cf, cmp, constant crates do NOT construct Yield/Return/Call variants. They only use Continue, Jump, Fork. No changes needed. |
| W11 (test updates) | Absorbed — each wave handles its own test updates as part of the implementation plan. No separate test-only wave needed. |

## Key Risk: Wave 1 Scope

Wave 1 is the largest single plan (W3+W4+W5+W6). It touches 16+ files across the interpreter framework, plus mechanical compat in kirin-scf, kirin-function, and example/toy-lang. The risk is manageable because:
1. All changes are mechanically driven by the Continuation enum change (change the type, fix all compiler errors).
2. The abstract interpreter changes (W6) are a natural extension of the same pattern.
3. All existing tests provide regression coverage.

If Wave 1 proves too large for a single agent, it can be split into:
- W1a: Continuation enum + concrete interpreter (control.rs, exec.rs, frame.rs, transition.rs, stage.rs)
- W1b: Abstract interpreter (result.rs, fixpoint.rs, interp.rs)

But W1b cannot start until W1a completes because it depends on the new Continuation variant shapes.
