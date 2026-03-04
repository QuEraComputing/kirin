# kirin-ir Review — 2026-03-02

**Scope:** `crates/kirin-ir/src/` — ~4,700 lines, 38 files
**Reviewers:** PL Theorist, Compiler Engineer, Rust Engineer, Physicist
**Plan:** docs/plans/2026-03-02-kirin-ir-review-plan.md

## Correctness & Safety

[P0] `Detach` does not decrement parent's `LinkedList::len` — corrupts `ExactSizeIterator::len()` on `StatementIter` after any detach. Downstream code relying on `size_hint` or `.len()` will silently produce incorrect results. — `detach.rs:14-57` [Rust Engineer]

[P0] `IdMap::get` panics on out-of-range IDs instead of returning `Option` — after `gc()`, any stale ID causes an unrecoverable panic. — `arena/gc.rs:9-13` [Rust Engineer]

[P0] [Won't Fix] `Arena::Index` can access deleted items without indication — `arena[id]` returns `&Item<T>` even if `item.deleted == true`. At minimum, add `debug_assert!(!self.items[...].deleted)` in `Index`/`IndexMut`. — `arena/data.rs:73-85` [Rust Engineer] — *Intentional: the deleted flag is needed for the rewrite framework which must access deleted items during rewrite passes.*

[P1] [Won't Fix] `LatticeSemantics::applicable` checks `call_param.is_subseteq(cand_param)` (covariant), but sound subtype dispatch requires contravariance for input positions. Verify the intended variance direction; variable naming and docs say "subtypes" but the code does supertypes. — `signature/semantics.rs:96-102` [PL Theorist] — *Incorrect: covariant check is correct for dispatch — the call site's type must be a subtype of the candidate's type to be applicable.*

## Abstractions & Type Design

[P1] [Won't Fix] `Successor` and `Block` share the same `Id` with free bidirectional conversion, collapsing a meaningful semantic distinction (control-flow edge vs. structural container) into a representational isomorphism. The newtype gives zero static safety. — `node/block.rs:27-37` [PL Theorist] — *Intentional: Successor and Block are separate newtypes with explicit conversion; the distinction is semantic and enforced at the API level.*

[P1] `SSAKind::Test`, `SSAKind::BuilderBlockArgument`, `SSAKind::BuilderResult`, and `TestSSAValue` are internal/transient states exposed in the public enum. These break the algebraic interpretation of `SSAKind` as `Result | BlockArgument`. Builder placeholders should be a separate type or behind `#[doc(hidden)]`. — `node/ssa.rs:113-125` [PL Theorist, Rust Engineer]

[P1] [Won't Fix] `TypeLattice` bundles `FiniteLattice + CompileTimeValue + Default` but has no inherent methods or blanket impl. It is a naked alias-trait with no laws beyond its components. Either give it algebraic content or replace with a where-clause bundle. — `lattice.rs:59` [PL Theorist] — *Intentional: TypeLattice is a semantic marker trait, not just an alias — it signals that the type participates in the lattice-based type system.*

[P2] [Won't Fix] `Dialect` is a god-trait (14 supertraits + 3 auto-trait bounds). Prevents implementing `Dialect` for types that structurally lack some capabilities. Derive macro mitigation is pragmatically adequate but limits compositionality. — `language.rs:79-99` [PL Theorist] — *Intentional: derive macro mitigates the boilerplate; all dialects use derive in practice.*

[P2] [Won't Fix] `SpecializedFunction` is `(StagedFunction, usize)` — a raw index, not an arena ID. Becomes invalid if specializations are reordered or removed. Lacks `Identifier` trait and cannot participate in arena GC. — `node/function/specialized.rs:10` [PL Theorist] — *Intentional: SpecializedFunction is always part of a StagedFunction; invalidation handles reordering.*

[P2] [Won't Fix] `BlockInfo<L>` carries `PhantomData<L>` but has no `L`-dependent fields. The phantom exists solely for `GetInfo<L>` dispatch — a type-level indirection without semantic content. — `node/block.rs:51-61` [PL Theorist] — *Intentional: PhantomData needed for GetInfo<L> dispatch.*

## Performance & Scalability

[P1] `Pipeline::lookup_symbol` allocates a `String` on every call via `name.to_string()` for `FxHashMap` lookup. For a hot path (symbol resolution), this is unnecessary. Fix: add `InternTable::lookup_by_ref` using `HashMap::get` with `Borrow` trait. — `pipeline.rs:114` [Compiler Engineer, Rust Engineer]

[P1] [Won't Fix] `Item<T>` has `deleted: bool` adjacent to `data: T` — for small `T`, this burns 7 bytes of padding per item. With thousands of SSA values, that's real cache pressure. Consider: move deleted bitset to a separate `BitVec` on `Arena`. — `arena/item.rs:5` [Compiler Engineer] — *Intentional: keep per-item deleted flag for rewrite framework which needs item-level deletion tracking.*

[P1] `all_matching` has O(n²) complexity — filters applicable candidates, then for each checks all specializations again. Re-computes `S::applicable` for the same candidates in the inner loop. — `node/function/staged.rs:102-136` [PL Theorist, Rust Engineer]

[P2] `Arena::alloc` uses `bon` builder for `Item::new` on hot allocation path. Verify it optimizes away, or use a plain struct literal. — `arena/data.rs:31-32` [Compiler Engineer]

[P2] [Won't Fix] `InternTable::intern` clones `T` unconditionally before insert — for `String` interning, every new symbol is cloned once for `Vec` and once for `HashMap`. Use `Rc<str>` or `HashMap::entry` to avoid double allocation. — `intern.rs:40` [Compiler Engineer] — *Not actionable: clone unavoidable without a data structure change (e.g., Rc<str>); current approach is simple and correct.*

[P2] [Won't Fix] `DenseHint` uses `Vec<Option<T>>` — for general `T`, every slot pays `size_of::<T>() + 1` with padding. A parallel `Vec<T>` with separate `BitVec` occupancy mask would be more cache-friendly. — `arena/hint/dense.rs:8` [Compiler Engineer] — *Accept trade-off: Vec<Option<T>> is simpler and the performance impact is minimal for current use cases.*

[P2] [Won't Fix] `StatementIter` chases linked-list pointers through the arena — poor spatial locality when statements aren't allocated in block order. Known trade-off; worth noting for future optimization. — `node/block.rs:174-196` [Compiler Engineer] — *Informational: known trade-off.*

[P3] `StageDispatch` does linear scan over the HList — O(N) with N dialects. For typical N (2-5), this is fine. No action needed. — `stage/dispatch.rs:33-38` [Compiler Engineer]

[P3] `Arena::gc` does two passes (map + retain) — could be single-pass, but GC is presumably rare. Fine as-is. — `arena/gc.rs:32-45` [Compiler Engineer]

## API Ergonomics & Naming

[P1] The three-level function hierarchy (Function → StagedFunction → SpecializedFunction) requires three separate API calls with two ID types before attaching any IR. No convenience method like `pipeline.simple_function("name", stage, body)` for the common case. — `pipeline.rs:207-268`, `builder/context.rs:230-272` [Physicist]

[P1] `lib.rs` exports 45+ names flat with no module grouping and no doc comment. No `prelude` module surfacing just the 8-10 names a dialect author needs. — `lib.rs:17-42` [Physicist]

[P2] `StageInfo::specialize()` uses single-letter parameter `f` while rest of API uses `func`. — `builder/context.rs:232` [Physicist]

[P2] `BlockBuilder::argument_with_name` takes `(name, ty)` while `BlockBuilder::argument` takes `(ty)` — inconsistent ordering. — `builder/block.rs:39-48` [Physicist]

[P2] `Pipeline::link()` panics on unknown Function but `Pipeline::staged_function()` auto-links — creates "which do I use?" confusion. — `pipeline.rs:135-140` [Physicist]

[P3] [Won't Fix] The test in `context.rs` manually implements all 14 Dialect supertraits for a trivial TestDialect — exactly the boilerplate wall a new user hits without `#[derive(Dialect)]`. A doc comment pointing to the derive macro would help. — `builder/context.rs:346-472` [Physicist] — *Intentional: tests avoid derive dependency to test bare-minimum logic.*

[P3] `SpecializedFunction::id()` returns `(StagedFunction, usize)` — a raw tuple. Named accessors (`.staged()`, `.index()`) would be more self-documenting. — `node/function/specialized.rs:13-15` [Physicist]

## Code Quality & Idioms

[P1] `detach.rs` uses `.and_then(|prev| { ...; Some(()) })` for side effects — should be `if let Some(prev) = prev { ... }`. Also uses `if let None = prev` instead of `prev.is_none()`. — `detach.rs:24-28,38,47` [Rust Engineer]

[P1] [Won't Fix] `link_statements`/`link_blocks` panic on doubly-linked nodes with `Debug` output of IR nodes. Library code should return `Result` instead. — `builder/context.rs:27,34,55,62` [Rust Engineer] — *Intentional: panics catch IR corruption during building stage; these indicate programming errors, not runtime conditions.*

[P2] `Arena` has `len()` but no `is_empty()` — clippy `len_without_is_empty`. — `arena/data.rs:24-26` [Rust Engineer]

[P2] `gc()` uses explicit `return IdMap(raw)` instead of implicit return. — `arena/gc.rs:45` [Rust Engineer]

[P2] `backedges()` and `specializations()` return `&Vec<T>` instead of `&[T]` — leaks container type. — `node/function/staged.rs:70,86`, `node/function/specialized.rs:87` [Rust Engineer]

## Cross-Cutting Themes

1. **Arena/deletion safety** — identified by 2 reviewers across Correctness and Performance. The `deleted` flag in `Item<T>` has both correctness issues (indexing returns deleted data) and performance costs (padding waste). A unified solution (separate bitset + debug assertions) addresses both.

2. **`SpecializedFunction` is under-typed** — identified by 3 reviewers across Abstractions, Ergonomics, and Correctness. It's a raw `(StagedFunction, usize)` tuple that lacks type safety (PL Theorist), named accessors (Physicist), and arena protocol participation (PL Theorist).

3. **Builder/test-only variants in public API** — identified by 2 reviewers across Abstractions and Code Quality. `SSAKind::Test`, `SSAKind::Builder*`, and `TestSSAValue` pollute the public API surface.

4. **String allocation on hot paths** — identified by 2 reviewers across Performance and Code Quality. Both `Pipeline::lookup_symbol` and `InternTable::intern` allocate unnecessarily.

5. **Missing convenience API for common function creation** — identified by Physicist and corroborated by PL Theorist's observation about SpecializedFunction's raw encoding. The 3-level function hierarchy is principled but lacks ergonomic shortcuts.

## Summary

- P0: 3 issues (must fix)
- P1: 10 issues (should fix)
- P2: 13 improvements (nice to have)
- P3: 7 notes (informational)
