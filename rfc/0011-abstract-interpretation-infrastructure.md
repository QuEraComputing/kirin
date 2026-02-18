+++
rfc = "0011"
title = "abstract interpretation infrastructure"
status = "Draft"
agents = ["claude"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-13T03:40:00.000000Z"
last_updated = "2026-02-13T12:00:00.000000Z"
+++

# RFC 0011: abstract interpretation infrastructure

## Summary

Add an `AbstractInterpreter` type implementing the `Interpreter` trait for automated abstract interpretation, built-in `Interpretable` implementations for `kirin-cf` and `kirin-function`, and a branching protocol that supports quantum-style global-state-aware control flow. This completes the abstract interpretation story begun in RFC 0009 and restructured by RFC 0010.

## Motivation

- Problem: The current framework provides `AbstractValue`/`Lattice` traits and `ExecutionControl::Fork`, but no automated fixpoint computation. Users must manually implement worklist iteration, widening/narrowing schedules, and block-state merging. Additionally, every user must re-implement `Interpretable` for `ControlFlow` and `FunctionBody` from scratch.
- Why now: RFC 0010 merged Session into StackInterpreter, establishing the pattern that the interpreter IS the thing that interprets. `AbstractInterpreter` follows the same pattern — a new type implementing `Interpreter` with a different walking strategy (worklist-based fixpoint instead of stack-based call/return).
- Stakeholders: `kirin-interpreter`, `kirin-cf`, `kirin-function`, analysis pass authors, quantum computing dialect authors.

## Goals

- `AbstractInterpreter`: worklist-based fixpoint computation with configurable widening strategy, implementing `Interpreter`.
- Built-in `Interpretable` for `kirin-cf::ControlFlow` and `kirin-function::{FunctionBody, Call, Return}` behind `interpret` feature flags.
- Branching protocol that supports: (a) deterministic concrete branching, (b) classical abstract interpretation with `Fork`, (c) quantum control flow where branch decisions depend on and modify global interpreter state.
- `AnalysisResult` type for querying fixpoint results by block or SSA value.

## Non-goals

- Backward analysis (reverse post-order traversal, liveness).
- Galois connection formalization.
- `Interpretable` derive macro.
- Sparse analysis (`SparseForwardAnalysis`).

## Guide-level Explanation

### Built-in Interpretable Impls

With the `interpret` feature enabled on `kirin-cf` and `kirin-function`, common dialect operations interpret themselves:

```rust
// No need to write match arms for Branch, Return, FunctionBody.
// Only write Interpretable for your custom dialect operations.
```

### Branching Protocol

Conditional branching is handled by the `Interpretable` implementation, not by the framework. This gives each domain full control:

- **Concrete**: Read the condition, pick one branch via `Jump`.
- **Classical abstract**: Return `Fork` with both targets when the condition is undecidable.
- **Quantum**: Read/modify global state (amplitudes, density matrices) through `I::global_mut()`, then return `Fork` with entanglement metadata encoded in the block arguments.

The `Interpretable` impl has access to `&mut I` (the full interpreter including global state), so domain-specific branching logic is naturally expressed without framework-level branching traits.

### AbstractInterpreter

```rust
let mut interp = AbstractInterpreter::new(&pipeline, stage)
    .with_widening(WideningStrategy::LoopHeaders)
    .with_max_iterations(100);

let result = interp.run_forward::<TestDialect>(entry_func, &initial_args)?;

// Query results
let val: &Interval = result.ssa_value(some_ssa);
let block_state: &BlockSnapshot<Interval> = result.block_entry(some_block);
```

`AbstractInterpreter` follows the same pattern as `StackInterpreter` — it implements `Interpreter` (read/write) and owns its execution strategy. But instead of a call stack, it owns a worklist, block-level state maps, and widening configuration.

The driver handles:
1. Seeding the entry block with initial abstract values.
2. Worklist processing: interpret blocks, propagate states via `Jump`/`Fork`.
3. Join/widen at merge points per the configured strategy.
4. Fixpoint detection (worklist empty).
5. Optional narrowing phase.

## Reference-level Explanation

### API and syntax changes

#### 1. Built-in Interpretable for kirin-cf

In `kirin-cf/Cargo.toml`:
```toml
[features]
interpret = ["kirin-interpreter"]
```

The `Interpretable` impl handles `Branch` (unconditional jump) and `Return` (return value). `ConditionalBranch` is deliberately excluded — its semantics are domain-dependent:

- Concrete `i64`: `if cond < 0 { Jump(true_target) } else { Jump(false_target) }`
- Abstract `Interval`: `if definitely_negative { Jump } else if definitely_non_negative { Jump } else { Fork }`
- Quantum: modify global amplitude state, Fork with entangled block args

```rust
#[cfg(feature = "interpret")]
impl<I, T> Interpretable<I> for ControlFlow<T>
where
    I: Interpreter,
    I::Value: Clone,
{
    fn interpret(&self, interp: &mut I) -> Result<ExecutionControl<I::Value>, I::Error> {
        match self {
            ControlFlow::Branch { target } => {
                Ok(ExecutionControl::Jump((*target).into(), vec![]))
            }
            ControlFlow::Return(value) => {
                let v = interp.read(*value)?;
                Ok(ExecutionControl::Return(v))
            }
            // ConditionalBranch: domain-dependent, handled by wrapping dialect
            _ => Ok(ExecutionControl::Continue),
        }
    }
}
```

#### 2. Built-in Interpretable for kirin-function

```rust
#[cfg(feature = "interpret")]
impl<I, T> Interpretable<I> for FunctionBody<T>
where
    I: Interpreter,
{
    fn interpret(&self, _interp: &mut I) -> Result<ExecutionControl<I::Value>, I::Error> {
        Ok(ExecutionControl::Continue)
    }
}
```

#### 3. AbstractInterpreter

New type in `kirin-interpreter` implementing `Interpreter`:

```rust
pub struct AbstractInterpreter<'ir, V, S, E = InterpError, G = ()>
where
    S: CompileStageInfo,
    V: AbstractValue,
{
    pipeline: &'ir Pipeline<S>,
    active_stage: CompileStage,
    global: G,

    // Current block being interpreted
    current_values: FxHashMap<SSAValue, V>,

    // Worklist and block-level state
    worklist: VecDeque<Block>,
    block_entry_states: FxHashMap<Block, FxHashMap<SSAValue, V>>,

    // Configuration
    widening_strategy: WideningStrategy,
    max_iterations: usize,
    narrowing_iterations: usize,

    _error: PhantomData<E>,  // E: InterpreterError bound on impl blocks
}

pub enum WideningStrategy {
    AllJoins,
    LoopHeaders,
    DelayedN(usize),
}

pub struct AnalysisResult<V> {
    block_entry: FxHashMap<Block, FxHashMap<SSAValue, V>>,
    ssa_values: FxHashMap<SSAValue, V>,
}
```

`AbstractInterpreter` implements `Interpreter` by reading/writing from `current_values` (the active block's SSA bindings). Its execution method `run_forward` drives the worklist loop:

```rust
impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone,
    S: CompileStageInfo,
{
    pub fn run_forward<L>(
        &mut self,
        entry: SpecializedFunction,
        initial_args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<Self>,
    {
        // 1. Seed entry block with initial abstract values
        // 2. Worklist loop:
        //    a. Pop block from worklist
        //    b. Load block's entry state into current_values
        //    c. Interpret all statements in block
        //    d. For Jump: join/widen incoming state with target, re-enqueue if changed
        //    e. For Fork: process all targets (same as Jump per-target)
        //    f. For Return/Halt/Break: record exit state
        // 3. Fixpoint = worklist empty
        // 4. Narrowing phase: re-traverse, applying narrow
    }
}
```

The key insight: `AbstractInterpreter` implements the same `Interpreter` trait as `StackInterpreter`, so existing `Interpretable` impls work with both. The difference is the walking strategy, not the value contract.

#### 4. AnalysisResult API

```rust
impl<V> AnalysisResult<V> {
    pub fn ssa_value(&self, ssa: SSAValue) -> Option<&V>;
    pub fn block_entry(&self, block: Block) -> Option<&FxHashMap<SSAValue, V>>;
    pub fn block_entries(&self) -> impl Iterator<Item = (Block, &FxHashMap<SSAValue, V>)>;
}
```

### Semantics and invariants

- **Branching is user-controlled**: The framework never decides which branch to take. `Interpretable` impls return `Jump` (deterministic), `Fork` (explore both), or any mix. This is essential for quantum dialects where branching involves global state mutation.
- **Fork semantics in AbstractInterpreter**: Each fork target gets a copy of the current abstract state, joined/widened with the target's existing entry state. For quantum, the global state is part of the value domain.
- **Widening at join points**: Applied when `is_subseteq` fails after joining, per the configured strategy.
- **Narrowing**: Bounded number of iterations after ascending chain stabilizes.
- **Same Interpreter trait**: Both `StackInterpreter` and `AbstractInterpreter` implement `Interpreter`. The trait is just read/write — the walking strategy is impl-specific.

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-interpreter` | Add `AbstractInterpreter`, `AnalysisResult`, `WideningStrategy` | New fixpoint tests |
| `kirin-cf` | Add `interpret` feature with `Interpretable` for `Branch`/`Return` | New integration test |
| `kirin-function` | Add `interpret` feature with `Interpretable` for `FunctionBody` | New integration test |

## Drawbacks

- **AbstractInterpreter complexity**: Worklist-based fixpoint computation is inherently complex. However, without it the abstract interpretation story is incomplete.
- **Feature flag dependency**: `kirin-cf` and `kirin-function` gain optional dependency on `kirin-interpreter`. This is a new inter-crate coupling, but feature-gated.
- **No ConditionalBranch built-in**: Users must still write their own conditional branch logic. This is intentional but increases boilerplate.

## Rationale and Alternatives

### Proposed: AbstractInterpreter as a new Interpreter impl

Follows the pattern established by RFC 0010: the interpreter IS the thing that interprets. `StackInterpreter` = stack-based concrete execution. `AbstractInterpreter` = worklist-based abstract execution. Same trait, different strategies.

### Alternative: FixpointDriver as a separate wrapper (like old Session)

- Description: A `FixpointDriver<I: Interpreter>` that wraps any interpreter and drives fixpoint iteration.
- Pros: Reuses existing `StackInterpreter` for state storage.
- Cons: Re-introduces the Session pattern we just eliminated. The walking strategy (worklist vs call stack) is fundamentally different — it's not just wrapping, it's a different kind of interpreter.
- Rejected: RFC 0010 established that the interpreter owns its strategy.

### Alternative: BranchCondition trait

```rust
trait BranchCondition {
    fn evaluate(&self) -> BranchDecision; // True, False, Both
}
```

- Pros: Uniform branching API.
- Cons: Cannot express quantum branching (needs global state mutation during evaluation). Forces all domains into a ternary decision model.
- Rejected: Too restrictive for quantum and other non-classical control flow.

## Prior Art

- **Frama-C / EVA**: Worklist-based abstract interpretation with configurable widening.
- **IKOS (NASA)**: Dense forward analysis framework. Inspiration for `WideningStrategy::LoopHeaders`.
- **Qiskit / Cirq**: Quantum circuit simulation where control flow decisions modify global state.
- **MLIR's dataflow analysis framework**: Region-based analysis with transfer functions per operation.

## Backward Compatibility and Migration

- No breaking changes to existing code. All additions are behind feature flags or new types.
- Users currently doing manual fixpoint iteration can migrate to `AbstractInterpreter` at their own pace.

## How to Teach This

- Add `examples/abstract_interval.rs`: Interval analysis of a loop using `AbstractInterpreter`.
- Add `examples/quantum_branching.rs`: Quantum control flow with global state demonstrating `Fork`.
- Document the branching protocol with three examples (concrete, abstract, quantum) in `AbstractInterpreter` doc comments.

## Reference Implementation Plan

1. **Slice 1**: Add `interpret` feature to `kirin-cf` and `kirin-function` with built-in `Interpretable` impls.
2. **Slice 2**: Implement `AbstractInterpreter` with `AllJoins` widening strategy. Test with interval domain on a simple loop.
3. **Slice 3**: Add `LoopHeaders` and `DelayedN` widening strategies. Add narrowing phase.
4. **Slice 4**: Add quantum branching example using global state.

### Acceptance Criteria

- [ ] `kirin-cf` provides `Interpretable` for `Branch`/`Return` behind `interpret` feature
- [ ] `kirin-function` provides `Interpretable` for `FunctionBody` behind `interpret` feature
- [ ] `AbstractInterpreter` computes correct fixpoint for `x = 0; while (x < 100) x = x + 1` with interval domain
- [ ] Narrowing refines `[0, +inf)` to `[0, 100]`
- [ ] `Fork` correctly explored in abstract conditional branch
- [ ] All existing tests pass

## Unresolved Questions

- **Widening point detection**: `LoopHeaders` requires identifying loop headers. Should this be a `kirin-ir` utility or computed by the interpreter?
- **Narrowing iteration bound**: Default 2-3 (sufficient for intervals) or configurable with convergence detection?
- **Call handling**: Should calls be inlined (analyze callee at each call site) or summarized (compute callee summary once, reuse at all call sites)?
- **Global state on fork**: When forking, should global state be cloned per-branch or shared? For quantum, cloned is correct (each branch has its own amplitude). For shared memory models, shared may be needed.

## Future Possibilities

- `SparseForwardAnalysis` tracking only def-use chains.
- `BackwardAbstractInterpreter` for reverse analyses.
- Galois connection traits for formal soundness verification.
- `Interpretable` derive macro for dialect enum dispatch.
- Inter-procedural analysis with call graph and function summaries.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-13 | RFC created. Split from RFC 0010 to keep scope focused. |
| 2026-02-13 | Rewritten: replaced `FixpointDriver` with `AbstractInterpreter` implementing `Interpreter`, aligning with RFC 0010's "interpreter IS the thing that interprets" pattern. |
