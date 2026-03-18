# RFC 0001: Phase-Separated IR — Build vs Finalized Stages

- **Status**: Draft
- **Authors**: roger, claude
- **Created**: 2026-03-17

## Summary

Split `SSAInfo` (and potentially other IR info types) into two phases: a **build-time** representation that allows `Option` types, `Unresolved` SSA kinds, and incomplete metadata; and a **finalized** representation where every field is guaranteed present and no build-time placeholders remain. This makes the phase distinction type-safe rather than relying on runtime assertions.

## Motivation

Today, `SSAInfo<L>` serves double duty:

```rust
pub struct SSAInfo<L: Dialect> {
    pub(crate) id: SSAValue,
    pub(crate) name: Option<Symbol>,
    pub(crate) ty: L::Type,        // ← must be present, even during building
    pub(crate) kind: SSAKind,      // ← may be Unresolved during building
    pub(crate) uses: SmallVec<[Use; 2]>,
}
```

During IR construction:
- The `ty` field may not be known yet (forward references in graph bodies with relaxed dominance). Currently we require `L::Type: Placeholder` to fill it with a sentinel, which forces this trait bound onto graph emit paths — even for type systems where "placeholder" has no meaning (e.g., a simple `{ I64, F64 }` enum).
- The `kind` field may be `SSAKind::Unresolved(ResolutionInfo)`, which must not appear in finalized IR but nothing in the type system prevents it.

During IR consumption (interpretation, analysis, rewrites):
- Consumers must match on `Unresolved` and `Test` variants even though they should never appear. This adds dead code and hides bugs if a builder fails to resolve.

### Concrete problem

Graph body emit (`DiGraph::emit_with`, `UnGraph::emit_with`) requires `IR::Type: Placeholder + Clone` in its where clause because `set_relaxed_dominance(true)` internally calls `L::Type::placeholder()` to create forward-reference SSAs. A dialect with a type system that has no natural placeholder value (e.g., `enum SimpleNumericType { I64, F64 }`) cannot use graph bodies without adding a synthetic `Placeholder` impl.

## Design

### Core idea: parameterize the phase

```rust
/// Marker for the building phase — types may be absent, kinds may be unresolved.
pub struct Building;

/// Marker for the finalized phase — all fields present, no unresolved kinds.
pub struct Finalized;

pub struct SSAInfo<L: Dialect, Phase = Finalized> {
    pub(crate) id: SSAValue,
    pub(crate) name: Option<Symbol>,
    pub(crate) ty: PhaseType<L::Type, Phase>,
    pub(crate) kind: PhaseKind<Phase>,
    pub(crate) uses: SmallVec<[Use; 2]>,
}
```

Where `PhaseType` and `PhaseKind` are type-level switches:

```rust
// Type field: Option during building, required when finalized
type PhaseType<T, Phase> = <Phase as PhaseSpec>::Type<T>;

trait PhaseSpec {
    type Type<T>;
    type Kind;
}

impl PhaseSpec for Building {
    type Type<T> = Option<T>;     // may be absent for forward refs
    type Kind = SSAKind;          // includes Unresolved variant
}

impl PhaseSpec for Finalized {
    type Type<T> = T;             // always present
    type Kind = FinalizedSSAKind; // no Unresolved, no Test
}
```

```rust
pub enum FinalizedSSAKind {
    Result(Statement, usize),
    BlockArgument(Block, usize),
    Port(PortParent, usize),
}
```

### Finalization step

A `finalize()` method on `StageInfo<L, Building>` produces `StageInfo<L, Finalized>`:

```rust
impl<L: Dialect> StageInfo<L, Building> {
    pub fn finalize(self) -> Result<StageInfo<L, Finalized>, FinalizeError> {
        // Walk all SSAs, verify no Unresolved kinds remain, all types present
        // Convert Building arenas to Finalized arenas
    }
}
```

This is the single validation checkpoint. If any `Unresolved` SSA or `None` type remains, `finalize()` returns an error with diagnostic info.

### What changes

| Component | Before | After |
|-----------|--------|-------|
| `SSAInfo` | Single type, runtime `Unresolved` check | Parameterized by phase |
| `StageInfo` | Single type | `StageInfo<L, Building>` for construction, `StageInfo<L, Finalized>` for consumption |
| Builders | Create `SSAInfo` with `Placeholder::placeholder()` | Create `SSAInfo<L, Building>` with `ty: None` |
| `SSAKind` | Has `Unresolved` + `Test` | Split into `SSAKind` (with `Unresolved`) and `FinalizedSSAKind` (without) |
| Interpreters/rewrites | Match on `Unresolved => unreachable!()` | Only see `FinalizedSSAKind` — exhaustive match on 3 variants |
| `Placeholder` bound | Required on graph emit paths | Not needed — `None` used instead |
| Forward-ref SSAs | `ty: L::Type::placeholder()` | `ty: None` |

### What doesn't change

- The `SSAValue`, `ResultValue`, `BlockArgument`, `Port` ID types — they're phase-independent arena indices.
- The `Pipeline` type — stages are already independently typed.
- Parser/printer APIs — they work with `StageInfo<L, Building>` internally, but the public roundtrip APIs can accept either phase via a trait bound.

## Crate impact

| Crate | Impact |
|-------|--------|
| `kirin-ir` | Core change: `SSAInfo`, `SSAKind`, `StageInfo`, all builders |
| `kirin-chumsky` | `EmitContext` works with `Building` phase; remove `Placeholder` bounds |
| `kirin-prettyless` | Printer works with `Finalized` phase (or trait-abstracted) |
| `kirin-interpreter` | Receives `Finalized` stage — no `Unresolved` matching |
| `kirin-derive-*` | Generated code targets `Building` phase for emit, `Finalized` for interpret |
| Dialect crates | No change if they don't interact with phase directly |

## Alternatives

### A: `Option<L::Type>` without phase parameterization

Make `SSAInfo.ty` an `Option<L::Type>` always. Simpler change, but:
- Every `ty()` consumer returns `Option<&L::Type>` — lots of unwrapping.
- No type-level guarantee that finalized IR has all types.
- `Unresolved` kind still lives in the same enum.

### B: Keep current design with Placeholder bound

Accept that graph body emit requires `Placeholder`. Dialect authors using graph bodies add `impl Placeholder for MyType`. This works today and is the status quo.

**Downside**: `Placeholder` is a type-system concept ("infer this later") being used for a build-time concern ("I haven't parsed the definition yet"). These are semantically different.

## Open questions

1. **Should `StageInfo` be the phase boundary, or should individual arenas be parameterized?** If only `SSAInfo` needs the phase, parameterizing `StageInfo` is overkill.

2. **Can we use GATs (Generic Associated Types) on a `Phase` trait instead of marker structs?** This avoids the extra type parameter on `SSAInfo` but requires careful lifetime handling.

3. **How does serialization work across phases?** `serde` derives would need to handle both phases. Likely only `Finalized` is serialized.

4. **Should `finalize()` be fallible or panicking?** Fallible is more correct but forces error handling at every pipeline stage boundary.

5. **Migration path**: Can we introduce the phase parameter with `Phase = Finalized` as default, making the change backwards-compatible for consumers? Only builders would need to opt into `Building`.
