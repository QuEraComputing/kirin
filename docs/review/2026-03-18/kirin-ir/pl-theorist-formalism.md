# PL Theorist — Formalism Review: kirin-ir

## Abstraction Composability

### Dialect trait as a product of capabilities

The `Dialect` trait (`language.rs:103-128`) requires all 14 accessor traits (`HasArguments`, `HasResults`, `HasBlocks`, `HasSuccessors`, `HasRegions`, `HasDigraphs`, `HasUngraphs`, plus their `Mut` variants) and 5 property traits (`IsTerminator`, `IsConstant`, `IsPure`, `IsSpeculatable`, `IsEdge`) as supertraits. This is a **closed-world product encoding** — every dialect must implement all capabilities, returning empty iterators for unused ones.

The alternative — a capability-based encoding where operations declare only the traits they use (e.g., `T: HasArguments + HasResults`) — would compose more flexibly: new capabilities could be added without modifying `Dialect`, and bounds would be minimal. The tradeoff is that downstream consumers would need to enumerate individual bounds rather than writing a single `L: Dialect`. The derive macro mitigates the boilerplate cost of the current approach, so the closed-world choice is defensible for a framework where the set of structural capabilities is known ahead of time.

**Metric**: Current approach requires 1 bound downstream (`L: Dialect`); open-world would require O(k) bounds where k is the number of capabilities used. However, the closed world prevents adding new capabilities without a breaking change to `Dialect`.

### Stage dispatch via type-level heterogeneous lists

`StageMeta::Languages` (`meta.rs:72`) uses nested tuples `(L, (M, ()))` as a type-level list. `StageDispatch` and `StageDispatchMut` (`dispatch.rs:10-120`) recurse over this list. This is the standard HList encoding from the Rust ecosystem (e.g., `frunk`). The recursion terminates at `()`.

This composes cleanly: `HasStageInfo<L>` can be implemented independently for each dialect, and `StageDispatch` composes via the tuple structure. The `SupportsStageDispatch` blanket (`dispatch.rs:53-73`) lifts the recursion into a single-bound constraint. The encoding has O(n) compile-time cost in the number of languages but is fine for small language counts.

### HasStageInfo: multiparamter projection

`HasStageInfo<L>` (`meta.rs:28-39`) uses the dialect type `L` as a parameter, enabling multiple implementations for enum stages. This is a clean encoding of a dependent-sum projection: given a stage enum `S`, `HasStageInfo<L>` extracts the `StageInfo<L>` component. The `try_` prefix returning `Option` correctly models the partial nature of the projection.

### Three-level function hierarchy

The `Function` -> `StagedFunction` -> `SpecializedFunction` hierarchy (`node/function/mod.rs:1-39`) maps to a standard refinement funnel: identity -> stage-specific representation -> type-specialized body. This aligns with MLIR's function separation across module passes, but adds the specialization level which corresponds to parametric polymorphism resolution (or Julia-style method specialization).

`SignatureSemantics` (`signature/semantics.rs:20-42`) parameterizes the dispatch policy. The two implementations — `ExactSemantics` and `LatticeSemantics` — cover nominal and structural subtyping models respectively. The `SignatureCmp` four-element lattice (More/Less/Equal/Incomparable) matches the standard partial-order comparison used in Haskell's type class resolution and Julia's method dispatch.

### Pipeline parameterized by stage type

`Pipeline<S>` (`pipeline.rs:21-26`) is generic over the stage container type `S`. This means `Pipeline<StageInfo<L>>` (single dialect) and `Pipeline<MyStageEnum>` (multi-dialect) use the same code paths. The `where S: HasStageInfo<L>` constraints ensure type safety at the call site. This is a clean parameterization.

## Literature Alignment

### MLIR correspondence

The Block/Region/Statement hierarchy closely follows MLIR's Block/Region/Operation model (Lattner et al., "MLIR: A Compiler Infrastructure for the End of Moore's Law", 2020):

- `Block` = MLIR `Block` (linear statement sequence + optional terminator + block arguments)
- `Region` = MLIR `Region` (ordered list of blocks via `LinkedList<Block>`)
- `Statement` = MLIR `Operation` (contains a dialect-specific `definition: L`)
- `SSAValue` / `ResultValue` / `BlockArgument` = MLIR `Value` / `OpResult` / `BlockArgument`

The naming divergence (`Statement` vs `Operation`) is minor but could cause confusion for users familiar with MLIR. MLIR uses "Operation" to avoid confusion with "statement" in imperative languages where statements don't produce values. In Kirin, statements produce `ResultValue`s, making the naming slightly inconsistent.

The `Successor` type (`block.rs:20-37`) reuses the `Block` ID space via `Successor::target() -> Block`. This matches MLIR's successor model where successors reference blocks within the same region.

### Lattice theory

The `Lattice` trait (`lattice.rs:28-32`) correctly specifies join, meet, and `is_subseteq` with the algebraic laws documented in comments (associativity, commutativity, idempotence, absorption, ordering consistency). The `HasBottom` and `HasTop` traits correctly factor the bounded lattice into composable pieces.

However, `TypeLattice` (`lattice.rs:59`) requires both `FiniteLattice` (bounded) and `Default`. The `Default` bound is semantically ambiguous — is the default value the bottom, top, or something else? If it's bottom, it duplicates `HasBottom::bottom()`. If it's something else, what is its lattice interpretation? This could lead to subtle bugs in dispatch.

### Arena-based IR representation

The use of arenas with ID-based references (`Arena<Statement, StatementInfo<L>>`) is a well-established pattern from ECS (Entity Component System) architecture and compiler IR design (Cranelift, MLIR). The `LinkedList` structure embedded in `StatementInfo` and `BlockInfo` provides O(1) insertion/deletion while maintaining arena ownership.

### Compile-time values

`CompileTimeValue` (`comptime.rs:1`) requires `Clone + Debug + Hash + PartialEq`. The blanket impl (`comptime.rs:50`) auto-implements it for all qualifying types. `Placeholder` extends this with a factory method for uninferred types. This models the pre-inference state cleanly — `Placeholder` is essentially the "top" element in an information lattice where "fully inferred" is the bottom.

## Semantic Ambiguity

### `BlockInfo::terminator` dual semantics

`BlockInfo::terminator` is described as "a cached pointer to the last statement" (`block.rs:59`, `AGENTS.md`), yet `StatementIter` excludes it from iteration (`block.rs:122-133`). This means the same `Statement` can exist both in the `statements` linked list and as the `terminator` pointer, but the iterator skips it. The `last_statement` method (`block.rs:161-164`) returns `terminator.or_else(|| statements.tail())`, which is correct but depends on the invariant that the terminator is always the last element if it exists. If the linked list is modified without updating the terminator cache, correctness breaks silently. The design is documented but the dual membership (in the linked list and as a cached field) creates a consistency invariant that is not enforced by the type system.

### `StageMeta::from_stage_name` blanket always succeeds

The `StageInfo<L>` impl of `StageMeta::from_stage_name` (`meta.rs:111-113`) always returns `Ok(StageInfo::default())` regardless of the `stage_name` argument. This means any stage name is accepted, which is inconsistent with the documented purpose ("build a concrete stage from a parsed stage name"). The enum-level `StageMeta` derives presumably validate, but the base case's permissiveness is surprising.

### `TypeLattice` includes `Default` but relationship to lattice is unspecified

`TypeLattice: FiniteLattice + CompileTimeValue + Default` (`lattice.rs:59`) — the relationship between `Default::default()` and `HasBottom::bottom()` / `HasTop::top()` is unspecified. If they are meant to coincide, one should be derived from the other.

## Alternative Formalisms Considered

### 1. Dialect: Product vs. Sum-of-capabilities

**Current**: Closed product — `Dialect` has all 19 supertraits.
**Alternative A**: Open trait families — `trait HasArguments + HasResults` per operation.
**Alternative B**: Capability-indexed approach — `trait Dialect<Caps: CapabilitySet>`.

| Metric | Product (current) | Open families | Capability-indexed |
|--------|-------------------|---------------|--------------------|
| Downstream bounds | 1 (`L: Dialect`) | O(k) per use site | 1 (`L: Dialect<Caps>`) |
| Adding new capability | Breaking change | Non-breaking | Non-breaking |
| Derive boilerplate | Low (auto-generated) | Medium | High |
| Compile-time cost | Low | Low | High (type-level computation) |

The product approach is the right choice for a framework with a stable, small set of capabilities where derive macros handle the boilerplate.

### 2. Stage dispatch: HList vs. enum-based

**Current**: Type-level HList `(L, (M, ()))` with recursive trait impls.
**Alternative A**: Runtime enum dispatch with `dyn StageInfo`.
**Alternative B**: Macro-generated match arms (no type-level recursion).

| Metric | HList (current) | Runtime enum | Macro match |
|--------|----------------|--------------|-------------|
| Type safety | Full static | Partial (downcasting) | Full static |
| Extensibility | Composable | Open | Open |
| Compile-time cost | O(n) recursion | O(1) | O(1) |
| Code clarity | Abstract | Concrete | Concrete |

HList is the principled choice for a fixed set of dialects per pipeline. The `#[derive(StageMeta)]` macro hides the abstraction cost from users.

### 3. Signature semantics: Trait vs. Type class dictionary

**Current**: `SignatureSemantics<T, C>` as a trait with `applicable` and `cmp_candidate` as associated functions.
**Alternative**: Pass a `DispatchPolicy` value at runtime (dictionary passing, as in Scala implicits or Haskell type class dictionaries).

The trait approach makes the semantics a compile-time choice, which is appropriate since a pipeline's dispatch strategy doesn't change at runtime. The type-level encoding also enables the compiler to monomorphize the dispatch logic.

## Summary

- [P2] [confirmed] `TypeLattice` requires `Default` without specifying its relationship to `bottom()` — `lattice.rs:59`
- [P3] [confirmed] `Statement` naming diverges from MLIR's `Operation` — `node/stmt.rs:12`
- [P3] [confirmed] `BlockInfo::terminator` cache consistency invariant is not type-enforced — `node/block.rs:59`
- [P3] [confirmed] `StageMeta::from_stage_name` blanket impl ignores the `stage_name` argument — `stage/meta.rs:111-113`
- [P3] [confirmed] Closed `Dialect` supertrait set prevents non-breaking capability extension — `language.rs:103-128`
