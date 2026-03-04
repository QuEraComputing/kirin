# kirin-ir Review — 2026-03-03

**Scope:** `crates/kirin-ir/src/` — ~4,800 lines, 47 files
**Reviewers:** PL Theorist, Compiler Engineer, Rust Engineer, Physicist
**Plan:** docs/plans/2026-03-03-kirin-ir-review-plan.md
**Prior review:** docs/reviews/2026-03-02-kirin-ir-review.md

## Correctness & Safety

No P0 or P1 issues found. The P0 fixes from the prior review (detach len, IdMap::get) are verified correct.

[P3] [confirmed] **Accepted** `detach.rs` len decrement will underflow if len is already zero. Should not happen with correct IR, but a `debug_assert!(parent_info.get_len() > 0)` before the decrement would guard against corruption propagation. — `detach.rs:53` [Rust Engineer]

## Abstractions & Type Design

[P2] [likely] **Accepted** `LatticeSemantics::applicable` checks parameter subtyping but ignores return type. `ExactSemantics` checks `call.ret == cand.ret`, but `LatticeSemantics` does not apply any return type check. A candidate returning `Bottom` matches a call expecting `Top`. Similarly, `LatticeSemantics::cmp_candidate` compares only params. — `signature/semantics.rs:92-102` [PL Theorist]
*User note: should check if return types match (error on mismatch) but return should NOT affect dispatch ordering.*

[P2] [likely] **Accepted** `LatticeSemantics::applicable` ignores the `constraints` field. `ExactSemantics` checks constraints equality, but `LatticeSemantics` silently drops them. Safe today because `C` defaults to `()`, but asymmetry with `ExactSemantics` is surprising and fragile if non-unit constraints are introduced. — `signature/semantics.rs:92-102` [PL Theorist]

[P2] [likely] **Accepted** `prelude` module omits `Signature` and `SignatureSemantics`. Dialect authors working with function dispatch will almost always need these. — `lib.rs:45-51` [PL Theorist]

## Performance & Scalability

[P2] [likely] **Accepted** `Pipeline::intern` takes `impl Into<String>`, unconditionally allocating a `String` before checking the intern table. Since `InternTable::lookup` now supports `&str` via `Borrow`, a two-phase approach (lookup first, intern on miss) would avoid allocation on cache hits. — `pipeline.rs:104` [Compiler Engineer]

[P3] [uncertain] **Accepted** `DenseHint::insert` silently drops values for out-of-range IDs. If an arena grows after the hint is created, `insert` discards the value because `get_mut` returns `None`. Could cause subtle missing-data issues under scaling. — `arena/hint/dense.rs:34-38` [Compiler Engineer]
*User note: should follow similar behavior as HashMap::insert.*

[P3] [uncertain] **Accepted** `SparseHint::insert_or_combine` double-hashes on the miss path. `get_mut` followed by `insert` hashes the key twice; the `Entry` API would eliminate the redundant hash. Minor given typical sizes. — `arena/hint/sparse.rs:34-41` [Compiler Engineer]

[P3] [uncertain] **Accepted** `define_function` clones `Signature` unnecessarily. `signature.clone()` is passed to both `staged_function` and `specialize`. Since `Signature` contains a `Vec`, this is a non-trivial clone. Could pass by reference for the second use. — `pipeline.rs:316` [PL Theorist]

## API Ergonomics & Naming

[P2] [likely] **Accepted** `arg_name()` silently no-ops when called without a preceding `argument()`. If call order is wrong (e.g., `.arg_name("y").argument(F64)`), the name is silently dropped. A `debug_assert!` would catch mistakes at build time. — `builder/block.rs:53-58` [Physicist, Rust Engineer]
*User note: arg_name reads ugly in the chain — wants a better API design.*

[P3] [likely] **Accepted** Prelude missing `Function` and `Signature`. When following the `define_function` example, both are immediately needed but not in the prelude. — `lib.rs:45-51` [Physicist]

[P3] [uncertain] **Accepted** `specialize().func(sf)` — the parameter name `func` accepts a `StagedFunction`, not a `Function`. Coming from `define_function` where `func` means the abstract `Function`, this naming collision is confusing. `staged` or `staged_func` would be clearer. — `builder/context.rs:231` [Physicist]

[P3] [uncertain] [Won't Fix] `StageInfo` arena accessors use inconsistent naming: five getters use `_arena()` suffix but the symbol intern table uses `symbol_table()`. — `context.rs:88-101` [Physicist]
*Rationale: it's not using an arena, it's using an intern table. The suffix correctly reflects the data structure type.*

## Code Quality & Idioms

[P3] [likely] **Accepted** `BlockBuilder::stmt` uses `.then(|| panic!(...))` and `BlockBuilder::terminator` uses `let _ = expr || { panic!(...) }` — both are non-standard patterns for conditional panics. Idiomatic form is `assert!(!cond, "...")`. — `builder/block.rs:72-77,87-92` [Rust Engineer]

[P3] [likely] **Accepted** `Arena::len` and `is_empty` count deleted items (tombstones), while `iter()` filters them out. This means `arena.len()` can be non-zero while `arena.iter().count()` is zero. Consider documenting that `len` includes deleted items. — `arena/data.rs:24-29` [Rust Engineer]

## Cross-Cutting Themes

1. **`arg_name` silent failure** — identified by 2 reviewers (Physicist, Rust Engineer) across Ergonomics and Safety. Both recommend a debug_assert to catch misuse.

2. **Prelude completeness** — identified by 2 reviewers (PL Theorist, Physicist) across Abstractions and Ergonomics. `Signature`, `Function`, and `SignatureSemantics` are missing for common workflows.

3. **`specialize().func()` naming** — identified by Physicist. The recent `f` → `func` rename creates a naming collision with the abstract `Function` concept. `staged` would be more precise.

4. **LatticeSemantics incompleteness** — identified by PL Theorist. Return type and constraints are not checked, unlike ExactSemantics. This asymmetry is fragile.

## Summary

- P0: 0 issues
- P1: 0 issues
- P2: 4 improvements — all accepted
- P3: 9 notes — 8 accepted, 1 won't fix

Confirmed: 2 | Likely: 8 | Uncertain: 3

## Filtered Findings

<details>
<summary>1 finding filtered</summary>

- [P3] StageInfo accessor naming inconsistency — [Won't Fix]: it's not using an arena, it's an intern table. The suffix correctly reflects the data structure type.
</details>
