# kirin-interpreter -- Final Review Report

Cross-referenced from 4 reviewer reports: PL Theorist (formalism), Implementer (code quality), Physicist (ergonomics), Compiler Engineer (cross-cutting).

## High Priority (P0-P1)

### P1-1: Inner dialect enums lack derive support for Interpretable + SSACFGRegion
**Source:** Physicist
**Confidence:** High (verified)
**Files:** `crates/kirin-function/src/interpret_impl.rs:81-116` (Lexical), `267-301` (Lifted)

`Lexical<T>` and `Lifted<T>` manually implement `Interpretable` and `SSACFGRegion` with pure delegation match arms (4 arms each, each just calling `op.interpret::<L>(interp)` or `op.entry_block(stage)`). Meanwhile, top-level language enums like `HighLevel` already use `#[derive(Interpretable, SSACFGRegion)]` successfully. Adding these derives to the inner dialect enum definitions in `crates/kirin-function/src/lib.rs:37-55` would eliminate ~100 lines of manual boilerplate. The `#[callable]` attribute already exists for marking SSACFGRegion-delegating variants.

**Blocker:** Need to verify the derive macros work with `#[wraps]` at enum level (which `Lexical` and `Lifted` use). The top-level enums use per-variant `#[wraps]`, which is a different code path in the derive. This may require a small derive macro enhancement.

## Medium Priority (P2)

### P2-1: Missing `#[must_use]` annotations across the crate
**Source:** Implementer
**Confidence:** High (verified -- zero `#[must_use]` in the entire crate)
**Files:** Crate-wide

Key candidates:
- `Continuation` enum itself (discarding a continuation is always a bug)
- `AnalysisResult::bottom()`, `::new()`, `::ssa_value()`, `::return_value()`, `::is_subseteq()`
- `StackInterpreter::new()`, `::with_fuel()`, `::with_max_depth()`
- `AbstractInterpreter::new()`
- `FrameStack::new()`, `::depth()`, `::is_empty()`

### P2-2: `active_stage_info` panic message lacks dialect type name
**Source:** Compiler Engineer
**Confidence:** High (verified)
**File:** `crates/kirin-interpreter/src/stage_access.rs:36`

The panic message is `"active stage does not contain StageInfo for this dialect"` -- it does not name which dialect `L` was requested or which stage was active. Adding `std::any::type_name::<L>()` and the stage ID would make mismatches trivially diagnosable.

### P2-3: `bind_block_args` allocates `Vec<SSAValue>` per call in hot path
**Source:** Compiler Engineer
**Confidence:** Likely
**File:** `crates/kirin-interpreter/src/block_eval.rs:44-48`

The default `bind_block_args` implementation collects block argument SSA values into a `Vec<SSAValue>` on every call. In abstract interpretation fixpoint loops, this is a per-iteration heap allocation. `SmallVec<[SSAValue; 4]>` would avoid allocation for the common case (most blocks have <= 4 arguments).

### P2-4: Repetitive where-clause boilerplate on manual Interpretable impls
**Source:** Physicist
**Confidence:** High (verified across kirin-arith, kirin-function, kirin-cf)

Every manual `Interpretable` impl repeats the identical 3-line where clause. This is a consequence of the L-on-method design (which is intentional and correct). Not directly fixable without a convenience macro, but documenting a copy-paste template or providing a `interpretable_where!` helper macro would reduce friction for dialect authors.

### P2-5: No convenience API to resolve function name to SpecializedFunction
**Source:** Physicist
**Confidence:** Likely
**File:** `crates/kirin-function/src/interpret_impl.rs:149-234` (Call::interpret)

The `Call::interpret` implementation performs a 6-step lookup chain (symbol table -> global symbol -> function -> function info -> staged function -> specialization) with 6 distinct error branches. This same resolution dance is needed by any operation that calls a function. A `Staged::resolve_callee(target_symbol)` method could encapsulate this.

Note: This finding straddles kirin-interpreter and kirin-function. The resolution logic lives in kirin-function but could be extracted to a helper on `Staged` or `StageAccess`.

## Low Priority (P3)

### P3-1: `AnalysisResult` has informal partial order (no `HasBottom`/`Lattice` impl)
**Source:** PL Theorist
**File:** `crates/kirin-interpreter/src/result.rs:38-44`

`AnalysisResult` has `bottom()` and `is_subseteq()` as inherent methods but does not implement the `HasBottom` or `Lattice` traits. The partial order is correct and convergence relies on value-level lattice properties. Formalizing this is low priority since `AnalysisResult` is an internal analysis artifact, not a user-facing lattice.

### P3-2: Manual `Debug`/`Clone` impls on `AnalysisResult` replaceable with derives
**Source:** Implementer
**File:** `crates/kirin-interpreter/src/result.rs:16-34`

The manual impls are functionally identical to what `#[derive(Debug, Clone)]` would produce. The bounds (`V: Debug`, `V: Clone`) are the same either way. Can be replaced with derives for cleanliness.

### P3-3: `Continuation<V, Ext>` could derive `Clone` conditionally
**Source:** Implementer
**File:** `crates/kirin-interpreter/src/control.rs:18`

Adding `#[derive(Clone)]` (which generates `Clone` when `V: Clone` and `Ext: Clone`) would allow cloning continuations for diagnostic/logging purposes.

### P3-4: `Staged` lifetime asymmetry documentation
**Source:** PL Theorist
**File:** `crates/kirin-interpreter/src/stage.rs:15-18`

`Staged<'a, 'ir, I, L>` has two lifetimes (`'a` for interpreter borrow, `'ir` for pipeline data). A brief doc comment clarifying it is a temporary scoped builder (not a persistent handle) would help.

### P3-5: Monomorphization pressure from `interpret<L>` and `eval_block<L>`
**Source:** Compiler Engineer
**File:** `crates/kirin-interpreter/src/interpretable.rs:17`

O(dialects x languages) monomorphized functions. Method bodies are small (match + delegate), so code size is manageable. No action needed unless compile times become a problem.

## Strengths

1. **Clean trait decomposition.** The ValueStore / StageAccess / BlockEvaluator / Interpreter layering is a textbook stratified interface. Each layer adds exactly one capability, there is no diamond inheritance, and downstream consumers can bind at the minimal level needed. (PL Theorist, Implementer)

2. **L-on-method technique is well-executed.** Breaking E0275 by moving `L` from trait to method is a genuine advance in Rust generic encoding. The coinductive resolution works correctly, and the derive macro hides the complexity from users. (PL Theorist, Compiler Engineer)

3. **Continuation algebra is well-designed.** The `Ext` type parameter for interpreter-specific variants (defaulting to `Infallible`) is a clean open-recursion pattern. The `SmallVec`-based `Args<V>` avoids allocation for common cases. (PL Theorist)

4. **Error types are thorough.** `InterpreterError` (9 variants) and `StageResolutionError` (8 variants) cover every resolution failure mode with sufficient diagnostic context. The `Custom(Box<dyn Error>)` escape hatch is well-placed. (Compiler Engineer)

5. **Abstract interpretation follows established theory.** Widening/narrowing contracts match Cousot & Cousot. Delayed widening follows Blanchet et al. Interprocedural analysis uses standard nested fixpoint with summary caching. (PL Theorist)

6. **Zero clippy suppressions.** The crate has no `#[allow(...)]` attributes. (Implementer)

7. **User-facing API is ergonomic.** `StackInterpreter::new` + `interp.in_stage::<L>().call(spec, &args)` is a two-line entry point. Lifetime inference handles the `Staged` builder -- users never write the type explicitly. (Physicist)

## Filtered Findings

- **"ValueStore/StageAccess impl duplication across StackInterpreter and AbstractInterpreter"** (Implementer P3) -- Filtered because: intentional per trait decomposition design. Each interpreter has different type parameters and field layouts. The duplication is minimal (5 one-liner delegations).

- **"Constructor `new` + `new_with_global` pattern duplication"** (Implementer P3) -- Filtered because: standard Rust builder pattern, not actionable.

- **"Builder method duplication (`with_max_depth`, `global`)"** (Implementer P3) -- Filtered because: different concrete types with different type parameters; extraction not worth the indirection.

- **"`InterpreterError` is not Clone"** (Implementer P3) -- Filtered because: intentionally contains `Box<dyn Error>` for user extensibility. Cloneability would require `Clone`-able custom errors, restricting the API.

- **"`Continuation::Fork` constructible in concrete interpreter context"** (PL Theorist P3) -- Filtered because: making it unrepresentable via phantom types would significantly complicate the type. The runtime panic is documented and `Fork` is only reachable from dialect impls that check `BranchCondition::is_truthy`, which returns `Some` for concrete values.

- **"'ir lifetime threading adds complexity"** (Physicist) -- Filtered because: explicitly accepted in the design context. Required for `&'ir`-lived pipeline references.

- **"Heavy dev-dependencies slow test compilation"** (Compiler Engineer P3) -- Filtered because: these are test-only dependencies and the set is the minimum needed for integration testing across dialects. No production impact.

- **"L type parameter confusing for leaf authors"** (Physicist P3) -- Filtered because: this is the intentional E0275 workaround. The confusion is real but the design is correct. Documentation improvement covered under P2-4.

## Suggested Follow-Up Actions

1. **[P1-1]** Investigate adding `#[derive(Interpretable, SSACFGRegion)]` to `Lexical<T>` and `Lifted<T>` in `crates/kirin-function/src/lib.rs`. Verify the derive works with enum-level `#[wraps]` and `#[callable]` on specific variants (e.g., `FunctionBody`, `Lambda`). If the derive doesn't support this yet, file it as a derive-macro enhancement.

2. **[P2-2]** Add `std::any::type_name::<L>()` to the panic message in `stage_access.rs:36`. One-line fix.

3. **[P2-1]** Add `#[must_use]` to `Continuation`, `AnalysisResult` constructors/accessors, interpreter constructors, and `FrameStack` methods. Mechanical change, low risk.

4. **[P2-3]** Replace `Vec<SSAValue>` with `SmallVec<[SSAValue; 4]>` in `bind_block_args` default impl (`block_eval.rs:44`). One-line change.

5. **[P2-5]** Extract the 6-step function resolution chain from `Call::interpret` into a reusable helper (e.g., `Staged::resolve_unique_callee(symbol) -> Result<SpecializedFunction>`). This benefits any dialect that introduces call-like operations.

6. **[P3-2, P3-3]** Replace manual `Debug`/`Clone` on `AnalysisResult` with derives; add conditional `Clone` derive to `Continuation`. Trivial cleanup.

7. **[P2-4]** Add a "Writing an Interpretable impl" section to crate-level docs showing the canonical where-clause template and explaining why `L` appears on the method.
