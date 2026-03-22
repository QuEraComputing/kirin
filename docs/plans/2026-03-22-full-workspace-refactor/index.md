# Full Workspace Refactor -- Plan Index

**Date:** 2026-03-22
**Review report:** `docs/review/2026-03-22/report.md`
**Pattern:** in-place + additive (SCF result values)
**Total findings addressed:** 111 accepted out of 115 total (1 P0, 18 P1, 51 P2, 41 P3)

---

## Dependency Graph

```
low-hanging-fruit (LHF-1..17, single agent, sequential)
       |
    wave-1 (derive codegen fixes — 1 agent)
       |
    wave-2 (core IR soundness — 1 agent)
       |
    wave-3a ──────── wave-3b
    (parser scope    (printer fixes
     guards)          — 1 agent)
    — 1 agent        |
       |              |
    wave-4 (interpreter improvements — 1 agent)
       |
    wave-5a ──────── wave-5b
    (SCF result      (dialect soundness
     values)          — 1 agent)
    — 1 agent        |
       |              |
    wave-6 (test cleanup — 1 agent)
```

Waves 3a and 3b run in parallel (different crates).
Waves 5a and 5b run in parallel (different crates).
Wave 6 depends on waves 5a and 5b (touches test files for SCF and bitwise).

## Low-Hanging Fruit

| # | Title | Finding | Crate | Effort |
|---|-------|---------|-------|--------|
| LHF-1 | Add #[must_use] to Continuation | P1-15 | kirin-interpreter | 5 min |
| LHF-2 | bat default-features = false | P1-16 | kirin-prettyless | 5 min |
| LHF-3 | Add interval re-exports | P1-22 | kirin-interval | 5 min |
| LHF-4 | Clear terminator cache in detach | P1-1 | kirin-ir | 10 min |
| LHF-5 | checked_sub for length decrement | P1-2 | kirin-ir | 10 min |
| LHF-6 | Restrict Arena::gc() visibility | P1-3 | kirin-ir | 5 min |
| LHF-7 | Refactor print_ports dedup | P1-18 | kirin-prettyless | 10 min |
| LHF-8 | Remove SparseHint Clone bounds | P2 | kirin-ir | 5 min |
| LHF-9 | #[must_use] render builders | P2 | kirin-prettyless | 5 min |
| LHF-10 | #[must_use] PipelineDocument | P2 | kirin-prettyless | 5 min |
| LHF-11 | #[must_use] parser error types | P2 | kirin-chumsky | 10 min |
| LHF-12 | #[must_use] arena/builder methods | P2 | kirin-ir | 15 min |
| LHF-13 | Remove debug println! | P3 | tests/ | 5 min |
| LHF-14 | Remove dead strip_trailing_ws | P2 | tests/ | 5 min |
| LHF-15 | Handle register_ssa Results | P2 | tests/ | 5 min |
| LHF-16 | Remove unused PrettyPrintExt imports | P2 | tests/ | 5 min |
| LHF-17 | Upgrade debug_assert to assert | P2 | kirin-interpreter | 5 min |

**Plan file:** `low-hanging-fruit.md`

## Wave 1

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| `wave-1/derive-codegen-fixes-plan.md` | Derive Codegen Fixes | P0-1, P1-10, P1-11 | Implementer | kirin-derive-ir, kirin-derive-toolkit |

## Wave 2

**Depends on:** Wave 1 complete and merged.

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| `wave-2/core-ir-soundness-plan.md` | Core IR Soundness | P1-4 | Implementer | kirin-ir |

## Wave 3

**Depends on:** Wave 2 complete and merged. Wave 3a and 3b run in parallel.

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| `wave-3/parser-scope-guards-plan.md` | Parser Scope Guards | P1-5, P1-6, P1-7, P1-9 | Implementer | kirin-chumsky |
| `wave-3/printer-fixes-plan.md` | Printer Fixes | P1-17, P2 (dedup, NaN) | Implementer | kirin-prettyless |

## Wave 4

**Depends on:** Wave 3 complete and merged.

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| `wave-4/interpreter-improvements-plan.md` | Interpreter Improvements | P1-12, P2 (perf, docs) | Implementer | kirin-interpreter |

## Wave 5

**Depends on:** Wave 4 complete and merged. Wave 5a and 5b run in parallel.

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| `wave-5/scf-result-values-plan.md` | SCF Result Values | P1-19, P1-20, P1-21 | Implementer | kirin-scf |
| `wave-5/dialect-soundness-plan.md` | Dialect Soundness | P2 (TryFrom, shifts, Lambda) | Implementer | kirin-arith, kirin-bitwise, kirin-function |

## Wave 6

**Depends on:** Waves 5a and 5b complete and merged.

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| `wave-6/test-cleanup-plan.md` | Test Cleanup | P2 UnitTy dedup, Theme H | Implementer | tests/, kirin-cf, kirin-scf, kirin-bitwise, kirin-cmp, kirin-function |

## Agent Assignments

| Agent Name | Role | Wave | Plan File | Files Touched |
|------------|------|------|-----------|---------------|
| lhf-agent | Implementer | 0 | `low-hanging-fruit.md` | control.rs, Cargo.toml, lib.rs, detach.rs, gc.rs, ir_render.rs, sparse.rs, pipeline.rs, error types, arena/builder, result.rs, test files |
| derive-fixer | Implementer | 1 | `wave-1/derive-codegen-fixes-plan.md` | has_signature.rs, helpers.rs |
| ir-soundness | Implementer | 2 | `wave-2/core-ir-soundness-plan.md` | dense.rs |
| parser-guard | Implementer | 3 | `wave-3/parser-scope-guards-plan.md` | emit_ir.rs, graphs.rs, blocks.rs, parse_text.rs |
| printer-fix | Implementer | 3 | `wave-3/printer-fixes-plan.md` | bat.rs, ir_render.rs, impls.rs |
| interp-improve | Implementer | 4 | `wave-4/interpreter-improvements-plan.md` | stage_access.rs, block_eval.rs, fixpoint.rs, lib.rs, frame_stack.rs |
| scf-values | Implementer | 5 | `wave-5/scf-result-values-plan.md` | scf/lib.rs, scf/interpret_impl.rs, scf/tests.rs, roundtrip/scf.rs |
| dialect-sound | Implementer | 5 | `wave-5/dialect-soundness-plan.md` | arith_value.rs, bitwise/interpret_impl.rs, lambda.rs |
| test-cleaner | Implementer | 6 | `wave-6/test-cleanup-plan.md` | cf/tests.rs, scf/tests.rs, bitwise/tests.rs, cmp/tests.rs, function/call.rs, function/ret.rs, roundtrip/*.rs |

**File disjointness check:**

- **Within Wave 3:** parser-guard touches kirin-chumsky; printer-fix touches kirin-prettyless. No overlap.
- **Within Wave 5:** scf-values touches kirin-scf; dialect-sound touches kirin-arith, kirin-bitwise, kirin-function. No overlap.
- **LHF vs Wave 1:** LHF touches helpers.rs only for the redundant condition fix -- MOVED to Wave 1. No overlap.
- **LHF vs Wave 3b:** Both touch ir_render.rs. LHF touches `print_ports` function only (LHF-7). Wave 3b touches name resolution helpers and NaN handling in a different file. LHF runs first, so changes are sequential. Minor overlap in ir_render.rs but different functions.
- **Wave 5a/5b vs Wave 6:** Wave 6 touches scf/tests.rs and bitwise/tests.rs for UnitTy dedup. Wave 5a touches scf/tests.rs for new tests. Wave 5b touches bitwise/interpret_impl.rs (not tests.rs). Wave 6 runs AFTER wave 5, so this is sequenced correctly.

## Verification Checkpoints

After each wave:
1. `cargo build --workspace`
2. `cargo nextest run --workspace`
3. `cargo test --doc --workspace`
4. `cargo clippy --workspace`

## Excluded Findings

| Finding | Reason |
|---------|--------|
| P1-8 | REMOVED -- factually incorrect (Item::unwrap is not Option::unwrap; iterator pre-filters deleted entries) |
| P1-13 | Won't fix -- expect_info panics are correct for invalid block IDs (compiler/interpreter programming error) |
| P1-14 | Won't fix -- function name says "expect"; panicking is expected behavior |
| P1-23 | Deferred to low-priority -- QubitType teaching example completeness (P3 after downgrade) |

## P2/P3 Findings Not Individually Planned

The following P2/P3 findings are addressed within wave plans as secondary items, or deferred:

**Addressed within plans:**
- P2 detach dedup, BFS dedup, builder rename -- partial coverage in LHF/Wave 2
- P2 crate-level #[allow] cleanup -- Wave 4 (interpreter), other crates in future pass
- P2 port_list/capture_list dedup (kirin-chumsky) -- future pass (parser combinator dedup)
- P2 DeriveContext ToTokens no-op -- future pass (derive-toolkit cleanup)
- P2 to_snake_case acronyms -- future pass
- P2 FieldData manual Clone -- future pass
- P2 is_type last-segment match -- future pass
- P2 BuilderPattern docs -- Wave 1 notes
- P2 Header dead fields -- future pass
- P2 token slice copy -- future pass (performance)
- P2 Signature void return -- future pass
- P2 identifier alloc -- future pass (performance)
- P2 Config pub fields -- future pass (style)
- P2 InherentImplTemplate/TypeDefTemplate dedup -- future pass
- P2 is_call_forwarding dedup -- future pass
- P2 __Phantom unreachable boilerplate -- future pass
- P2 For induction_var docs -- addressed in Wave 5a
- P2 kirin-function empty tests -- partial in Wave 6
- P2 representative_intervals dedup -- future pass
- P2 ir_fixtures boilerplate -- future pass
- P2 Token::to_tokens verbosity -- future pass

**All P3 findings:** Accepted as low-priority, addressed opportunistically within wave plans or deferred.
