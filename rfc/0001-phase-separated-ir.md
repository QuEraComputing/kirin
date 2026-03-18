# RFC 0001: Phase-Separated IR — Build vs Finalized Stages

- **Status**: Draft
- **Authors**: roger, claude
- **Created**: 2026-03-17

## Summary

Split the IR into two phases: a **build-time** representation (internal, used by parsers and builders) and a **finalized** representation (public, used by interpreters, rewrites, and dialect authors). The finalized types keep clean, familiar names (`SSAKind`, `SSAInfo`, `StageInfo`). The build-time types are internal and qualified (`BuilderSSAKind`, `BuilderSSAInfo`).

## Motivation

Today, `SSAInfo<L>` serves double duty — construction and consumption share the same type:

```rust
pub struct SSAInfo<L: Dialect> {
    ty: L::Type,      // must be present, even during building
    kind: SSAKind,    // may be Unresolved during building
    // ...
}
```

**Build-time problems:**
- Forward references in graph bodies need an SSA before its type is known. Currently this requires `L::Type: Placeholder` to fill the type slot with a sentinel — forcing the bound onto graph emit paths, even for type systems where "placeholder" has no meaning (e.g., `{ I64, F64 }`).
- `SSAKind::Unresolved(ResolutionInfo)` is a build-time concern that leaks into the public enum.

**Consumption-time problems:**
- Interpreters and rewrite passes must match on `SSAKind::Unresolved` and `SSAKind::Test` even though they should never appear. This adds dead arms and hides builder bugs.
- `SSAInfo.ty()` is always `&L::Type` but could in principle be uninitialized if a builder forgot to resolve a forward ref.

### Design principle

**Downstream developers should only see finalized types.** The building stage is internal plumbing — it should be hidden behind parser/builder APIs that produce clean, validated IR.

## Design

### Naming convention

The public API uses clean names. Build-time internals are prefixed:

| Public (finalized) | Internal (build-time) |
|--------------------|-----------------------|
| `SSAKind` | `BuilderSSAKind` |
| `SSAInfo<L>` | `BuilderSSAInfo<L>` |
| `StageInfo<L>` | `BuilderStageInfo<L>` |

Downstream code never writes `BuilderSSAKind` — it's `pub(crate)` or gated behind a `builder` module.

### `SSAKind` — the clean, downstream enum

```rust
/// The kind of an SSA value in finalized IR.
/// Exhaustive — no hidden variants, no build-time placeholders.
pub enum SSAKind {
    Result(Statement, usize),
    BlockArgument(Block, usize),
    Port(PortParent, usize),
}
```

Three variants. Exhaustive `match`. No `_ =>` needed.

### `BuilderSSAKind` — internal build-time enum

```rust
/// SSA kind during IR construction. Includes unresolved placeholders.
#[doc(hidden)]
pub(crate) enum BuilderSSAKind {
    /// Already resolved to a final kind.
    Resolved(SSAKind),
    /// Placeholder — will be resolved when the enclosing builder finalizes.
    Unresolved(ResolutionInfo),
    /// Test-only placeholder.
    Test,
}
```

Builders create `BuilderSSAKind::Unresolved(...)`. When the builder finalizes, it resolves to `BuilderSSAKind::Resolved(SSAKind::Result(...))`. The `finalize()` step unwraps `Resolved` and rejects any remaining `Unresolved`.

### `SSAInfo` vs `BuilderSSAInfo`

```rust
/// Finalized SSA info — type always present, kind always resolved.
pub struct SSAInfo<L: Dialect> {
    id: SSAValue,
    name: Option<Symbol>,
    ty: L::Type,         // ← always present
    kind: SSAKind,       // ← no Unresolved
    uses: SmallVec<[Use; 2]>,
}

/// Build-time SSA info — type may be absent, kind may be unresolved.
pub(crate) struct BuilderSSAInfo<L: Dialect> {
    id: SSAValue,
    name: Option<Symbol>,
    ty: Option<L::Type>, // ← None for forward refs (no Placeholder needed!)
    kind: BuilderSSAKind,
    uses: SmallVec<[Use; 2]>,
}
```

### `StageInfo` vs `BuilderStageInfo`

```rust
/// Finalized stage — all SSAs resolved, all types present.
/// Used by interpreters, printers, and rewrite passes.
pub struct StageInfo<L: Dialect> {
    ssas: Arena<SSAValue, SSAInfo<L>>,
    // ... blocks, regions, digraphs, ungraphs — all finalized
}

/// Build-time stage — used by parsers and builders.
/// Contains BuilderSSAInfo with Option types and Unresolved kinds.
pub(crate) struct BuilderStageInfo<L: Dialect> {
    ssas: Arena<SSAValue, BuilderSSAInfo<L>>,
    // ... same structure but with builder info types
}

impl<L: Dialect> BuilderStageInfo<L> {
    /// Validate and convert to finalized StageInfo.
    /// Errors if any SSA has Unresolved kind or None type.
    pub fn finalize(self) -> Result<StageInfo<L>, FinalizeError> { ... }
}
```

### How the parser uses this

```rust
// Parser creates a BuilderStageInfo, emits IR, then finalizes
let mut builder_stage: BuilderStageInfo<MyDialect> = BuilderStageInfo::default();
let mut ctx = EmitContext::new(&mut builder_stage);
// ... emit statements, graphs, blocks ...

// Finalize: validates all SSAs are resolved, types present
let stage: StageInfo<MyDialect> = builder_stage.finalize()?;
```

The `EmitContext` works with `BuilderStageInfo`. Forward-ref SSAs use `ty: None` — no `Placeholder` bound needed anywhere.

### What downstream developers see

```rust
// Interpreter code — only uses StageInfo and SSAKind
fn interpret(stage: &StageInfo<L>, stmt: Statement) {
    let ssa_info = stmt.results(stage).next().unwrap();
    match ssa_info.kind() {
        SSAKind::Result(stmt, idx) => { ... }
        SSAKind::BlockArgument(block, idx) => { ... }
        SSAKind::Port(parent, idx) => { ... }
        // No Unresolved! No Test! Exhaustive.
    }
}
```

### Migration path

1. Default type parameter: `SSAInfo<L>` stays as the finalized type (no second parameter).
2. `BuilderSSAInfo<L>` is a new, separate type — not a parameterization of `SSAInfo`.
3. Public APIs (`ParseStatementText`, `ParsePipelineText`) continue to return `StageInfo<L>` — the builder stage is internal to the parser.
4. Existing code that creates `StageInfo` directly (tests, manual builders) gets thin wrappers or uses `BuilderStageInfo` + `finalize()`.

### What simplifies for downstream

| API | Before | After |
|-----|--------|-------|
| `SSAKind` match | 5 arms (Result, BlockArgument, Port, Unresolved, Test) | 3 arms (Result, BlockArgument, Port) |
| `SSAInfo.ty()` | `&L::Type` (but could be placeholder) | `&L::Type` (guaranteed real) |
| Graph body emit | Requires `L::Type: Placeholder` | No Placeholder bound |
| `Interpretable` impls | Must handle Unresolved panic | Impossible state eliminated |
| `PrettyPrint` impls | Must handle Unresolved | Clean match only |

## Crate impact

| Crate | Impact | Visibility |
|-------|--------|------------|
| `kirin-ir` | Core: add `BuilderSSAInfo`, `BuilderSSAKind`, `BuilderStageInfo`, `finalize()` | Internal |
| `kirin-chumsky` | `EmitContext` uses `BuilderStageInfo`; remove `Placeholder` bounds; `ParseStatementText` calls `finalize()` | Internal |
| `kirin-prettyless` | No change — works with `StageInfo` (finalized) | None |
| `kirin-interpreter` | Remove `Unresolved => unreachable!()` arms | Simplification |
| `kirin-derive-*` | Emit codegen targets `BuilderStageInfo`; interpret codegen targets `StageInfo` | Internal |
| Dialect crates | No change — public API unchanged | None |

## Alternatives

### A: Single `SSAInfo<L, Phase>` with generic parameter

Use a phase marker instead of separate types. Cleaner DRY but forces `Phase` parameter everywhere `SSAInfo` appears — cascades through `StageInfo`, `Arena`, `Pipeline`, every trait bound.

### B: `Option<L::Type>` without separate types

Make `SSAInfo.ty` an `Option<L::Type>` always. Every consumer unwraps. No type-level phase separation.

### C: Keep current design

Accept the `Placeholder` bound and `Unresolved` matching. Simplest, least work. Status quo.

## Open questions

1. **Shared ID space**: `BuilderSSAInfo` and `SSAInfo` use the same `SSAValue` IDs. The `finalize()` step just converts info types in the arena. Does this require arena changes?

2. **Incremental building**: Some workflows add to a stage after it's finalized (e.g., specialization). Should `StageInfo` support re-entering build mode?

3. **Printer during build**: The printer is useful during debugging before finalization. Should it work with `BuilderStageInfo` too (handling `None` types gracefully)?

4. **`Test` SSAKind**: Currently used for test fixtures. Should tests use `BuilderStageInfo` directly, or should there be a test-only mechanism in `SSAInfo`?
