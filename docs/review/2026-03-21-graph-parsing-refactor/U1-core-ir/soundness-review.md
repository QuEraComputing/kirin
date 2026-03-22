# U1: Core IR -- Soundness Review

## Invariant Inventory

| Invariant | Location | Enforcement |
|-----------|----------|-------------|
| port_name/capture_name requires preceding port/capture | builder/digraph.rs:48, ungraph.rs:45 | Debug-only |
| Block statements must not be terminators | builder/block.rs:69 | Runtime-always (assert!) |
| Block terminator must be a terminator | builder/block.rs:83 | Runtime-always (assert!) |
| Builder key index in bounds | builder/mod.rs:53 | Runtime-always (assert!) |
| Linked-list: no duplicate next/prev links | builder/stage_info.rs:534,541,562,569 | Runtime-always (panic!) |
| UnGraph edge SSA used by at most 2 nodes | builder/ungraph.rs:211 | Runtime-always (panic!) |
| SSA exists for operands during graph build | builder/digraph.rs:146, ungraph.rs:145 | Runtime-always (expect) |
| Deleted arena items never dereferenced | builder/stage_info.rs:222,272 | Caller's responsibility |
| Detach head/tail consistency | detach.rs:38-56 | Debug-only |
| Detach parent length > 0 before decrement | detach.rs:53 | Debug-only |
| Arena index in bounds | arena/data.rs:182 | Runtime-always (Vec index panic) |

## Findings

### [P0] [likely] `mem::zeroed()` on `SSAInfo<L>` creates invalid `SmallVec` -- builder/stage_info.rs:222,262,272

**Invariant:** `SmallVec<[Use; 2]>` must be validly initialized; a zeroed SmallVec has zeroed length/capacity fields but its internal discriminant (inline vs heap) may be in an invalid state depending on the SmallVec version.

**Enforcement:** Caller's responsibility (SAFETY comments claim deleted items are never dereferenced).

**Attack:** The `data` field of a deleted `Item<SSAInfo<L>>` is zeroed. However, `Arena::get(id)` returns `Option<&Item<T>>` for *any* id including deleted ones (confirmed in test at data.rs:238). If a caller reads a deleted item via `arena.get(stale_id)`, the `Deref` to `SSAInfo` provides access to a zeroed `SmallVec` and a zeroed `L::Type`. For any `L::Type` where zero-bytes is not a valid representation (e.g., `NonZero*`, enums with discriminants, `String`), this is immediate UB through safe code. Even if `SmallVec` inline mode happens to tolerate zero-init, `L::Type` is an unconstrained generic.

**Consequence:** UB through safe Rust API when a stale ID is used on a finalized `StageInfo`.

**Reachability:** Requires holding a deleted SSA ID and calling `arena.get()` or `GetInfo::get_info()` on finalized stage. Stale IDs are trivially obtainable by saving an SSAValue before the builder deletes it during placeholder resolution.

**Suggested mitigation:** Use `Arena::try_map_live_option` (already exists) to produce `Arena<I, Option<SSAInfo<L>>>`. Or wrap deleted items in `MaybeUninit`. The `finalize_unchecked` path at line 262 is even worse since it creates zeroed *live* items for SSAs lacking types.

### [P1] [confirmed] `finalize_unchecked` creates zeroed live `SSAInfo` for type-less SSAs -- builder/stage_info.rs:256-268

**Invariant:** Live SSAInfo items should have valid types and kinds.

**Enforcement:** Not enforced (`pub(crate)` escape hatch).

**Attack:** `StageInfo::with_builder` round-trips through `finalize_unchecked`. If a builder operation creates an SSA without setting a type (e.g., forward-reference creator in emit_ir.rs:128 sets `ty: None`), the resulting `SSAInfo<L>` has a zeroed `ty` field. Downstream code calling `.ty()` on that SSA gets a zeroed `L::Type`.

**Consequence:** Silent data corruption; zeroed type value returned as if valid.

**Reachability:** Normal use -- any `with_builder` call after parser creates forward-reference SSAs.

**Suggested mitigation:** Track unresolved SSAs through `with_builder` and either resolve them or error.

### [P3] [confirmed] `port_name`/`capture_name` silently ignored in release builds -- builder/digraph.rs:48,64; builder/ungraph.rs:45,59

**Invariant:** `port_name()` should only be called after `port()`.

**Enforcement:** Debug-only (`debug_assert!`). In release, calling `port_name("x")` on an empty ports vec is a no-op (the `if let Some(last)` guard catches it), but the name is silently dropped.

**Attack:** `DiGraphBuilder::new().port_name("x").port(ty).new()` -- the name "x" is silently lost.

**Consequence:** Silent name loss; not a crash but violates user intent.

**Reachability:** Normal use with incorrect builder call order.

**Suggested mitigation:** Promote to `assert!` or return `Result`.

## Strengths

- Block builder uses `assert!` (not `debug_assert!`) for terminator/non-terminator validation, preventing structural corruption.
- Finalize validation (`finalize()`) properly checks all live SSAs for unresolved kinds and missing types before conversion.
- Linked-list linking in `link_statements`/`link_blocks` panics on double-linking, preventing cycle creation.
- Builder key resolution uses `assert!` for bounds checking, preventing out-of-bounds port/capture access.
