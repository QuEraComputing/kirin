# kirin-ir -- Final Review Report

Consolidated from 4 reviewer perspectives: PL Theorist (formalism), Implementer (code quality), Physicist (ergonomics/DX), Compiler Engineer (cross-cutting concerns).

---

## High Priority (P0-P1)

### 1. No convenience methods for Pipeline stage/function resolution
**Severity:** P1 | **Confidence:** confirmed | **Source:** Physicist

Looking up a stage by name requires 8 lines of chained Option navigation (`example/toy-lang/src/main.rs:76-88`). Resolving a function name to a callable specialization takes 4 sequential lookups across 20 lines (`example/toy-lang/src/main.rs:91-110`). Every downstream consumer of `Pipeline` will repeat this pattern.

**Suggested API additions on `Pipeline<S>`:**
- `fn stage_by_name(&self, name: &str) -> Option<CompileStage>`
- `fn resolve_function(&self, name: &str, stage: CompileStage) -> Option<StagedFunction>` (or similar)

These are pure convenience wrappers over existing data; no architectural change needed.

---

## Medium Priority (P2)

### 2. DiGraphBuilder / UnGraphBuilder duplicated port/capture allocation
**Severity:** P2 | **Confidence:** confirmed | **Source:** Implementer

`crates/kirin-ir/src/builder/digraph.rs:84-183` and `crates/kirin-ir/src/builder/ungraph.rs:77-181` share nearly identical code for: port SSA creation, capture SSA creation, `port_name_to_index` / `capture_name_to_index` HashMap construction, replacement map building/application, and placeholder SSA deletion. The shared portion is roughly 80-100 lines.

The divergence begins after replacement application -- DiGraphBuilder builds a directed petgraph while UnGraphBuilder builds an undirected petgraph with BFS reordering. A shared `GraphPortAllocator` helper could extract the common prefix.

### 3. Missing `#[must_use]` annotations across the crate
**Severity:** P2 | **Confidence:** confirmed | **Source:** Implementer

Zero `#[must_use]` annotations exist in kirin-ir. Key candidates:
- `Id::raw()`, `Signature::placeholder()` -- pure constructors/accessors whose return values should never be discarded
- `GetInfo` trait methods (`get_info`, `expect_info`)
- `BuilderStageInfo::finalize()` returning `Result<StageInfo<L>, FinalizeError>`

### 4. `TypeLattice` requires `Default` without specifying relationship to lattice bounds
**Severity:** P2 | **Confidence:** confirmed | **Source:** PL Theorist, Implementer (partial overlap)

`crates/kirin-ir/src/lattice.rs:59`: `pub trait TypeLattice: FiniteLattice + CompileTimeValue + Default {}`

The `Default` bound is ambiguous -- is `Default::default()` the same as `HasBottom::bottom()`? If yes, one should derive from the other. If no, the lattice interpretation of the default value is unspecified, which could cause subtle bugs in dispatch logic (`LatticeSemantics` at `crates/kirin-ir/src/signature/semantics.rs:90` uses `TypeLattice`).

### 5. Missing `#[diagnostic::on_unimplemented]` on key traits
**Severity:** P2 | **Confidence:** likely | **Source:** Compiler Engineer

`AsBuildStage` already has an excellent `#[diagnostic::on_unimplemented]` message. The same pattern should be applied to:
- `HasStageInfo<L>` (`crates/kirin-ir/src/stage/meta.rs:28`) -- "stage enum does not contain StageInfo for dialect L; add a variant wrapping StageInfo<L>"
- `Dialect` (`crates/kirin-ir/src/language.rs:103`) -- "use #[derive(Dialect)] to implement all required IR accessor traits"
- `StageMeta` (`crates/kirin-ir/src/stage/meta.rs:68`) -- "use #[derive(StageMeta)] on your stage enum"

### 6. `bon` dependency brings duplicate `darling` version into the workspace
**Severity:** P2 | **Confidence:** confirmed | **Source:** Compiler Engineer

`bon` pulls in `darling 0.20` while the workspace uses `darling 0.23` via `kirin-derive-toolkit`. Since `kirin-ir` is the root of the dependency graph, this compile-time cost cascades everywhere. `bon` is used for only 4 builder methods on `Pipeline`. Hand-written builders on these methods would eliminate the extra proc-macro compilation.

### 7. PhantomData boilerplate on every generic dialect type
**Severity:** P2 | **Confidence:** confirmed | **Source:** Physicist

Every generic dialect enum needs a `__Phantom(PhantomData<T>)` variant. Every generic dialect struct needs `#[kirin(default)] marker: PhantomData<T>`. Examples: `kirin-arith/src/lib.rs:128`, `kirin-cf/src/lib.rs:47`, `kirin-scf/src/lib.rs:64,82,93`. The derive macro could auto-generate this when `T` appears in `#[kirin(type = T)]` but not in any field.

---

## Low Priority (P3)

### 8. `StageMeta::from_stage_name` blanket impl ignores argument
**Source:** PL Theorist, Compiler Engineer (duplicate)

`crates/kirin-ir/src/stage/meta.rs:111-113`: The `StageInfo<L>` impl always returns `Ok(StageInfo::default())` regardless of the `stage_name` argument. This is the base case for single-dialect pipelines where there is only one possible stage, so the permissiveness is correct in context. Enum-level `StageMeta` derives (the multi-dialect case) do validate. Low priority but worth a doc comment explaining why.

### 9. `StageMeta::from_stage_name` returns `String` error
**Source:** Compiler Engineer

A structured error type would enable better diagnostics. However, downstream `FunctionParseError` in `kirin-chumsky` already provides typo suggestions via `strsim`, so the practical impact is limited.

### 10. `Signature` fields are `pub`
**Source:** Implementer

`crates/kirin-ir/src/signature/signature.rs:7-11`: All three fields (`params`, `ret`, `constraints`) are public. This prevents future invariant enforcement. Low priority since the struct is simple and no invariants are currently needed.

### 11. `module_inception` allow in signature module
**Source:** Implementer

`crates/kirin-ir/src/signature/mod.rs:2`: Module `signature` contains `signature.rs`. Rename to `definition.rs` or `types.rs`.

### 12. Builder APIs use panics where Results could work
**Source:** Implementer

`crates/kirin-ir/src/builder/block.rs:69-73,83-87` and `crates/kirin-ir/src/builder/mod.rs:53-68` use `assert!`/`panic!` for validation. These are construction-time programming errors, so panics are defensible.

### 13. Long derive lists on every dialect type
**Source:** Physicist

8-10 derives per type. The standard Rust derives (`Debug, Clone, PartialEq, Eq, Hash`) are required by `Dialect`'s supertraits. A shorthand macro or having `#[derive(Dialect)]` auto-derive them could reduce noise, but this is cosmetic.

### 14. HRTB supertrait pressure on `Dialect`
**Source:** Compiler Engineer, PL Theorist (partial overlap)

19 `for<'a>` supertraits on `Dialect` contribute to trait-solver cost. Acceptable because derive handles implementation and users only write `L: Dialect`, but worth monitoring as the project scales.

---

## Strengths

- **`AsBuildStage` diagnostic hint** (Compiler Engineer): The `#[diagnostic::on_unimplemented]` message is a model for other traits. Clear, actionable error messages for common misuse patterns.

- **Lattice trait formalization** (PL Theorist): The `Lattice`, `HasBottom`, `HasTop`, `FiniteLattice` decomposition is algebraically correct with well-documented laws.

- **Arena-based IR scales well** (Compiler Engineer): O(1) lookups via `Arena<K, V>`, O(1) function lookup via `FxHashMap`, O(1) symbol resolution via `InternTable`.

- **`Pipeline<S>` generic design** (PL Theorist): Single-dialect (`Pipeline<StageInfo<L>>`) and multi-dialect (`Pipeline<MyStageEnum>`) use the same code paths. Clean parameterization.

- **Derive-heavy path hides complexity** (Physicist): Users encounter zero lifetime annotations on dialect definitions and minimal boilerplate. The concept budget for "add a new dialect" is ~13 concepts, and for "compose existing dialects" is ~7 concepts -- both manageable.

- **`SignatureSemantics` design** (PL Theorist): Clean separation of exact vs. lattice-based dispatch with a proper `SignatureCmp` partial order. Matches established PL literature.

- **Structured error types** (Compiler Engineer): `FinalizeError`, `PipelineError`, `StageDispatchMiss` all provide actionable, structured error information.

---

## Filtered Findings

- **"BlockInfo::terminator cache consistency invariant not type-enforced"** (PL Theorist P3) -- Filtered because: this is a documented intentional design decision in AGENTS.md. The terminator is explicitly described as "a cached pointer to the last statement" with specific usage patterns. The dual-membership design is the intended architecture.

- **"Closed Dialect supertrait set prevents non-breaking capability extension"** (PL Theorist P3) -- Filtered because: the PL Theorist report itself acknowledges this is defensible given the derive macro and stable capability set. The AGENTS.md documents derive as the primary path. This is an architectural choice, not a defect.

- **"Statement naming diverges from MLIR's Operation"** (PL Theorist P3) -- Filtered because: naming is an explicit project choice. Kirin is inspired by MLIR but is not an MLIR port. The name "Statement" is used consistently throughout.

- **"`#[wraps]` per-variant vs enum-level: two ways to do the same thing"** (Physicist P3) -- Filtered because: AGENTS.md explicitly documents the interaction between per-variant and enum-level `#[wraps]`, including when each is appropriate. This is intentional composability.

- **"Tuple-based StageDispatch is O(N) per dispatch"** (Compiler Engineer P3 informational) -- Filtered because: the report itself notes N is small in practice and the design is documented. Not actionable.

- **"`unit_cmp` allows in signature/semantics.rs"** (Implementer P2) -- Downgraded to filtered. The allows are on generic code where `C` defaults to `()`. The suggested fix (non-`()` marker) would add complexity for no practical benefit since `()` is the intended default constraint type.

---

## Suggested Follow-Up Actions

1. **Add `Pipeline` convenience methods** for stage-by-name lookup and function resolution. This is the highest-impact change for downstream usability. Target: `crates/kirin-ir/src/pipeline.rs`.

2. **Add `#[must_use]` annotations** to pure accessors and constructors (`Id::raw()`, `Signature::placeholder()`, `GetInfo` methods, `BuilderStageInfo::finalize()`).

3. **Extract shared port/capture allocation** from `DiGraphBuilder`/`UnGraphBuilder` into a helper.

4. **Clarify `TypeLattice::Default` semantics** -- either document the relationship to `bottom()`, derive one from the other, or remove `Default` in favor of `HasBottom`.

5. **Add `#[diagnostic::on_unimplemented]`** to `HasStageInfo`, `Dialect`, and `StageMeta`, following the `AsBuildStage` pattern.

6. **Evaluate removing `bon` dependency** in favor of hand-written builders on the 4 `Pipeline` methods, to eliminate the duplicate `darling` version.

7. **Auto-generate PhantomData** in the Dialect derive macro when the type parameter appears in `#[kirin(type = T)]` but not in any field.

8. **Add doc comment** to `StageInfo<L>::from_stage_name` explaining why it ignores the argument (single-dialect base case).
