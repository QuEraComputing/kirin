# U1: Core IR -- Formalism Review

## Findings

### [P0] [confirmed] `mem::zeroed()` on generic `SSAInfo<L>` in `finalize_unchecked` -- builder/stage_info.rs:261

`finalize_unchecked` uses `unsafe { std::mem::zeroed() }` to construct an `SSAInfo<L>` when `ty` is `None`. The `ty` field has type `L::Type` where `L: Dialect` and `Type: CompileTimeValue`. `CompileTimeValue` implies `Clone + PartialEq + Debug + Display + Hash + Eq`. Many valid `L::Type` implementations (e.g., `String`, `Vec<T>`, any heap-allocating type) have all-zeros representations that are *not valid values* -- a zeroed `String` has a null pointer that will segfault on drop. This is instant UB the moment the SSA arena is dropped, even if the item is "tombstoned," because `Arena::drop` drops all live items and the tombstone invariant is not enforced by the type system.

**Alternative formalisms:**

| Approach | Safety | Perf impact | Complexity |
|----------|--------|-------------|------------|
| `MaybeUninit<SSAInfo<L>>` + forget on drop | Sound | Zero-cost | Moderate -- need custom drop |
| `Option<SSAInfo<L>>` in the arena slot | Sound | +8 bytes/slot (niche opt) | Low |
| Require `L::Type: Default` on `finalize_unchecked` | Sound | Zero-cost | Low -- adds one bound |

**Suggested action:** Replace `mem::zeroed()` with `Option`-wrapping in the arena, or add a `Default` bound to the `finalize_unchecked` path. The first option is safest; deleted/invalid entries should be `None`, not zeroed bit patterns.

**References:** Rustonomicon, "Working with Uninitialized Memory"; MIR safety checks for `mem::zeroed` (rust-lang/rust#66151).

### [P2] [likely] `DiGraphInfo<L>` and `UnGraphInfo<L>` are structurally near-duplicates -- node/digraph.rs, node/ungraph.rs

Both structs share `id`, `parent`, `name`, `ports`, `edge_count`, `graph`, `PhantomData<L>` and differ only in the petgraph edge directedness (`Directed` vs `Undirected`) and one field (`yields` vs `edge_statements`). From a categorical perspective, both are instances of a `GraphInfo<L, D: EdgeType>` parameterized by directedness, with the divergent fields as an associated sum type. The 280+ lines of builder duplication (`DiGraphBuilder::new` vs `UnGraphBuilder::new`) is a maintenance liability -- identical port/capture resolution logic is copy-pasted.

**Alternative formalisms:**

| Approach | Lines saved | Extensibility | Compile-time |
|----------|-------------|---------------|-------------|
| `GraphInfo<L, D: EdgeType, Extra>` generic struct | ~200 | High (new graph kinds) | Slight increase (monomorphization) |
| Trait-based `GraphBody` abstracting shared methods | ~120 | Moderate | Neutral |
| Status quo (duplicated) | 0 | Low (N copy-paste sites) | Neutral |

**Suggested action:** Extract a `GraphInfo<L, D, Extra>` struct parameterized by `petgraph::EdgeType` and an extra-fields type. Refactor the shared builder logic into a generic `GraphBuilder<L, D>` with a trait for the divergent finalization step.

**References:** Wadler, "Theorems for free!" (parametricity over phantom types); petgraph's `EdgeType` trait.

## Strengths

- The `SSAKind` / `BuilderSSAKind` separation cleanly models the phase distinction (construction vs finalized IR) and the `finalize()` method enforces the invariant at the type level.
- `Signature<T, C>` is parametrically polymorphic in both the type domain and constraint context, with `SignatureSemantics` providing a clean typeclass for dispatch semantics. The lattice-based specialization ordering is correct.
- The `Dialect` supertrait bundle with `for<'a>` HRTB iterators is a principled encoding of an open family of IR properties, avoiding the n-squared trait-pair explosion.
