# Implementation Fixes — Plan Index

**Date:** 2026-03-22
**Review report:** `docs/plans/2026-03-22-full-workspace-refactor/implementation-notes.md`
**Pattern:** in-place
**Total findings addressed:** 4 accepted out of 10 total

---

## Dependency Graph

```
low-hanging-fruit (LHF-1: Staged Debug)
       |
    wave-1 (parallel)
       ├── cf-roundtrip (Stream B)
       └── void-if-yield (Stream C)
```

## Low-Hanging Fruit

| # | Title | Finding | Crate | Effort |
|---|-------|---------|-------|--------|
| LHF-1 | Add Debug impl for Staged | #7 | kirin-interpreter | ~15 min |

**Plan file:** `low-hanging-fruit.md`

## Wave 1

| Plan File | Title | Finding(s) | Agent Role | Crate(s) |
|-----------|-------|------------|------------|----------|
| `wave-1/cf-roundtrip-plan.md` | CF Roundtrip Mismatch Fix | #1 | Implementer | kirin-prettyless, tests |
| `wave-1/void-if-yield-plan.md` | Void If/For + Yield Enforcement | #3, #4 | Implementer | kirin-scf, kirin-interpreter |

## Agent Assignments

| Agent Name | Role | Wave | Plan File | Files Touched |
|------------|------|------|-----------|---------------|
| LHF Agent | Implementer | LHF | `low-hanging-fruit.md` | `kirin-interpreter/src/stage.rs`, `kirin-interpreter/src/stage_access.rs` |
| CF Roundtrip Agent | Implementer | 1 | `wave-1/cf-roundtrip-plan.md` | `kirin-prettyless/src/impls.rs`, `kirin-prettyless/src/document/ir_render.rs`, `kirin-prettyless/src/tests/impls.rs`, `tests/roundtrip/cf.rs` |
| Void If Agent | Implementer | 1 | `wave-1/void-if-yield-plan.md` | `kirin-scf/src/lib.rs`, `kirin-scf/src/interpret_impl.rs`, `kirin-interpreter/src/block_eval.rs` (doc only), `tests/roundtrip/scf.rs` |

**File disjointness check:** Confirmed — no file overlaps within Wave 1. The LHF agent touches only kirin-interpreter/src/stage.rs and stage_access.rs. The CF Roundtrip agent touches only kirin-prettyless files and tests/roundtrip/cf.rs. The Void If agent touches only kirin-scf files, kirin-interpreter/src/block_eval.rs (doc comment only), and tests/roundtrip/scf.rs. Zero shared files between any pair of agents.

## Verification Checkpoints

After each wave:
1. `cargo build --workspace`
2. `cargo nextest run --workspace`
3. `cargo test --doc --workspace`
4. `cargo clippy --workspace`

## Excluded Findings

| Finding | Reason |
|---------|--------|
| #2 (Vec\<ResultValue\> not supported) | Deferred — tuple packing design decision is adequate for current use cases. Full multi-result support requires derive macro changes (~3-4 days). |
| #5 (RAII scope guard borrow conflicts) | Solved during original refactor — `Deref`/`DerefMut` on guard types. |
| #6 (Constant From→TryFrom cascade) | Solved during original refactor — blanket `TryFrom` impl covers all callers. |
| #8 (Worktree isolation: agents in wrong repos) | Tooling issue, not code — mitigated by WORKTREE CHECK invariant in agent prompts. |
| #9 (run_forward frame pop contract) | Solved during original refactor — `run_forward` now pops frame on success. |
| #10 (Pre-existing clippy warnings) | No-op — `IdMap` dead code warnings are expected since GC infrastructure exists but isn't used externally yet. |
