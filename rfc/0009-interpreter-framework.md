+++
rfc = "0009"
title = "interpreter framework"
status = "Implemented"
agents = ["codex", "claude"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-12T16:09:01.905258Z"
last_updated = "2026-02-13T03:15:52.071005Z"
+++

# RFC 0009: interpreter framework

## Summary

Add a first-class interpreter framework to Kirin with a common
`Interpreter` trait for frame/value state management and a default concrete
implementation `StackInterpreter<V, E, G = ()>`, plus extension traits:
`Interpretable<I>`, `AbstractValue`, and `AbstractInterpreter`.
The `Interpreter` trait defines a state contract around reading SSA values,
writing statement results, and pushing/popping call frames. Execution logic
(statement stepping, call dispatch, breakpoints) lives in
`Session<'ir, I, S>` (where `I: Interpreter`), which owns the interpreter and
a reference to the `Pipeline<S>`. Having the full pipeline available at the
session level is essential because call conventions may need to resolve
abstract functions to concrete `SpecializedFunction` targets, which requires
pipeline-level function lookup. The session dispatches to dialect semantics by
retrieving the dialect definition from `Statement::definition(stage)` and
calling `Interpretable::interpret`. The framework also provides an explicit
reusable
`Frame<V>` type for downstream interpreters backed by `fxhash::FxHashMap`,
avoiding stage-wide dense allocation (SSA IDs are dense per-stage but sparse
per-function frame).

## Motivation

- Problem: Kirin IR models structure (`StageInfo`, `Statement`, `SSAValue`,
  `StagedFunction`) but does not provide a standardized execution interface.
  Today, `StageInfo` is a storage/container API (`crates/kirin-ir/src/context.rs`)
  and IR traversal is exposed as low-level iterators like
  `Block::statements`/`Block::terminator` (`crates/kirin-ir/src/node/block.rs`).
  This makes each interpreter or analysis engine invent its own runtime model.
- Why now: Recent work around function/stage semantics (for example RFC 0008)
  makes call-frame and function-level execution boundaries more important; we
  need one reusable interface before building concrete and abstract evaluators.
- Stakeholders:
  - dialect crate maintainers (`kirin-function`, `kirin-cf`, `kirin-scf`,
    arithmetic/constant/bitwise crates)
  - `kirin-ir` maintainers and pass authors
  - users building interpreters, evaluators, and static analyses on Kirin IR

## Goals

- Define a common `Interpreter` trait for frame/value access and call-frame
  transitions (state management only; execution logic lives in session types).
- Define a default stack-based interpreter type (`StackInterpreter<V, E, G = ()>`) as
  the primary runtime state container.
- Define a dialect hook trait (`Interpretable<I>`) so each dialect can
  provide operation semantics independently.
- Define an `AbstractValue` trait extending `Lattice`
  (`crates/kirin-ir/src/lattice.rs`) with a required `widen` method and a
  default no-op `narrow` method for abstract interpretation. Document
  algebraic contracts (widening monotonicity and chain stabilization,
  narrowing bounds) on the trait. No blanket implementation — every abstract
  value type must explicitly define its own widening operator.
- Define two abstract-interpreter modes:
  - dense forward data-flow over program points
  - sparse forward data-flow seeded from a specific `SSAValue`
- Provide a single pipeline-aware `Session<'ir, I, S>` type generic over any
  `I: Interpreter`, holding the full `Pipeline` for function resolution and
  cross-stage call dispatch. All execution APIs (`step`, `advance`, `run`,
  `run_until_break`, `call`) live on `Session`. This means the same session
  driver works for both concrete (`StackInterpreter`) and abstract
  (`AbstractInterpreter` impls) execution.
- Provide an explicit shared `Frame<V>` type so interpreter implementations can
  reuse a common call-frame representation.
- Use `fxhash::FxHashMap` for frame value storage to keep interface simple
  while avoiding stage-wide dense memory costs (SSA IDs are stage-dense but
  function-sparse).
- Specify the dispatch mechanism: session retrieves the dialect instance from
  `stmt.definition(stage)` and calls `Interpretable::interpret`, achieving
  static dispatch through the dialect enum type `L`.
- Keep the API additive and avoid breaking existing crates.

## Non-goals

- Defining a bytecode format or JIT backend.
- Replacing existing `Dialect` trait contracts in `kirin-ir`.
- Changing text parser/printer grammar in `kirin-chumsky` or
  `kirin-prettyless`.
- Defining optimization passes or CFG scheduling policy in this RFC.

## Guide-level Explanation

A runtime author can target the `Interpreter` trait and reuse or wrap
`StackInterpreter<V, E, G = ()>` for their value domain, error model, and
optional global runtime state. The `Interpreter` trait is a pure state
contract — it manages frames and SSA value bindings but does not contain
execution logic. A dialect author implements `Interpretable<I>` for statement
semantics. All execution is driven by `Session`, which owns the interpreter
and holds the full `Pipeline` — enabling function resolution and cross-stage
call dispatch at the same level where statement stepping happens.

Illustrative core traits:

A frame is the unit of call-local execution state. It binds SSA values produced
while executing one `SpecializedFunction` and tracks the statement cursor for
that activation. We use `FxHashMap` to keep the representation sparse: SSA value
IDs are dense per compile stage (allocated from a single stage-wide arena in
`StageInfo<L>`), but a function frame only touches a small non-contiguous subset
of those IDs. A `Vec<Option<V>>` indexed by raw ID would waste memory
proportional to the full stage arena. `FxHashMap` naturally handles the
"sparse subset of a dense global space" pattern.

```rust
pub struct Frame<V> {
    callee: SpecializedFunction,
    values: FxHashMap<SSAValue, V>,
    cursor: Option<Statement>,
}

impl<V> Frame<V> {
    pub fn new(
        callee: SpecializedFunction,
        cursor: Option<Statement>,
    ) -> Self {
        Self {
            callee,
            values: FxHashMap::default(),
            cursor,
        }
    }

    pub fn cursor(&self) -> Option<Statement> { self.cursor }
    pub fn set_cursor(&mut self, cursor: Option<Statement>) { self.cursor = cursor; }

    pub fn read(&self, value: SSAValue) -> Option<&V> {
        self.values.get(&value)
    }

    pub fn write(&mut self, result: ResultValue, value: V) -> Option<V> {
        self.values.insert(result.into(), value)
    }
}
```

`Frame<V>` is the **sole owner of the statement cursor**. The cursor tracks
which statement is next in the current activation. When a call frame is pushed,
the current cursor is saved in the caller's frame; when popped, it is restored.
Session types read the cursor from the interpreter's current frame rather than
maintaining their own copy.

`Interpreter` defines the state contract: frame access, SSA read/write, and
call-frame transitions. It does **not** contain execution entrypoints
(`execute_statement`, `call`) — those belong to the session types which have
access to both IR storage and interpreter state.

```rust
pub trait Interpreter {
    type Value;
    type Error;

    fn current_frame(&self) -> Result<&Frame<Self::Value>, Self::Error>;
    fn current_frame_mut(&mut self) -> Result<&mut Frame<Self::Value>, Self::Error>;
    fn unbound_value_error(&self, value: SSAValue) -> Self::Error;

    /// Returns a reference to the bound value without cloning.
    /// Useful for inspection, debugging, and cases where ownership is not needed.
    fn read_ref(&self, value: SSAValue) -> Result<&Self::Value, Self::Error> {
        self.current_frame()?
            .read(value)
            .ok_or_else(|| self.unbound_value_error(value))
    }

    /// Returns a cloned copy of the bound value.
    /// Preferred in dialect `interpret` impls where the caller needs ownership
    /// (e.g., to pass values to `write` or return them in `ExecutionControl`).
    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error>
    where
        Self::Value: Clone,
    {
        self.read_ref(value).cloned()
    }

    fn write(&mut self, result: ResultValue, value: Self::Value) -> Result<(), Self::Error> {
        self.current_frame_mut()?.write(result, value);
        Ok(())
    }

    fn push_call_frame(&mut self, frame: Frame<Self::Value>) -> Result<(), Self::Error>;
    fn pop_call_frame(&mut self) -> Result<Frame<Self::Value>, Self::Error>;
}
```

`StackInterpreter<V, E, G = ()>` is the default runtime state container
implementing `Interpreter`. It owns the call stack (`Vec<Frame<V>>`) and
provides default behavior for frame/value operations.

```rust
pub struct StackInterpreter<V, E, G = ()> {
    frames: Vec<Frame<V>>,
    global: G,
    unbound_value_error: fn(SSAValue) -> E,
}

impl<V, E> StackInterpreter<V, E, ()> {
    pub fn new(unbound_value_error: fn(SSAValue) -> E) -> Self { /* ... */ }
}

impl<V, E, G> StackInterpreter<V, E, G> {
    pub fn with_global(global: G, unbound_value_error: fn(SSAValue) -> E) -> Self { /* ... */ }
    pub fn global(&self) -> &G { /* ... */ }
    pub fn global_mut(&mut self) -> &mut G { /* ... */ }
}

impl<V, E, G> Interpreter for StackInterpreter<V, E, G>
where
    V: Clone,
{
    type Value = V;
    type Error = E;

    fn current_frame(&self) -> Result<&Frame<V>, E> { /* ... */ }
    fn current_frame_mut(&mut self) -> Result<&mut Frame<V>, E> { /* ... */ }
    fn unbound_value_error(&self, value: SSAValue) -> E { /* ... */ }
    fn push_call_frame(&mut self, frame: Frame<V>) -> Result<(), E> { /* ... */ }
    fn pop_call_frame(&mut self) -> Result<Frame<V>, E> { /* ... */ }
}
```

`Interpretable<I>` is the dialect hook, parameterized on the full
interpreter type `I`. Each dialect operation defines how it affects
interpreter state and which control action should happen next. The session
dispatches to the correct implementation by calling
`stmt.definition(stage)` to retrieve the dialect enum value `&L`, then
calling `L::interpret(interpreter)`. This is static dispatch through the
dialect enum — no runtime registry is needed.

```rust
pub trait Interpretable<I>: Dialect
where
    I: Interpreter,
{
    fn interpret(
        &self,
        interpreter: &mut I,
    ) -> Result<ExecutionControl<I::Value>, I::Error>;
}
```

**Parameterization on `I` (not `V`)** is deliberate. Dialect impls place
trait bounds on either `I::Value` (for pure transfer functions) or on `I`
directly (for side-effectful operations). This gives one trait and one
dispatch path while letting pure operations be generic across all
interpreters that share a value domain:

```rust
// Pure operation: bounds on I::Value only — works for any interpreter
// whose value type supports arithmetic (concrete i64, abstract Interval, etc.)
impl<I> Interpretable<I> for Add
where
    I: Interpreter,
    I::Value: ArithmeticValue,
{
    fn interpret(&self, interp: &mut I) -> Result<ExecutionControl<I::Value>, I::Error> {
        let a = interp.read(self.lhs)?;
        let b = interp.read(self.rhs)?;
        interp.write(self.result, a.add(&b))?;
        Ok(ExecutionControl::Continue)
    }
}

// Side-effectful operation: bounds on I directly — needs interpreter access
impl<I> Interpretable<I> for Print
where
    I: Interpreter,
    I: HasGlobal<StdoutBuffer>,
{
    fn interpret(&self, interp: &mut I) -> Result<ExecutionControl<I::Value>, I::Error> {
        let val = interp.read(self.operand)?;
        interp.global_mut().write_fmt(format_args!("{val}"));
        Ok(ExecutionControl::Continue)
    }
}
```

`AbstractValue` extends the existing `Lattice` trait
(`crates/kirin-ir/src/lattice.rs`) with `widen` (required) and `narrow`
(default no-op). The abstract value IS the domain element — there is no
separate "domain context" object. Every abstract value type must explicitly
define its own widening operator to guarantee termination. Even for finite
lattices, practical height may require widening distinct from `join` (e.g., a
powerset lattice over a large set).

**Algebraic contracts (must hold for all `x`, `y`):**

- **Widening**: `x ⊑ widen(x, y)` and `y ⊑ widen(x, y)`. The ascending
  chain `x₀, widen(x₀, x₁), widen(widen(x₀, x₁), x₂), ...` must stabilize
  in finite steps. This guarantees fixpoint termination.
- **Narrowing**: `x ⊓ y ⊑ narrow(x, y) ⊑ x`. Narrowing refines a
  post-fixpoint downward without going below the greatest fixpoint. The
  descending chain must also stabilize in finite steps.

These laws are documented on the trait and verified by a property-testing
harness in `kirin-test-utils` (see Acceptance Criteria).

```rust
pub trait AbstractValue: Lattice {
    /// Widen `self` with `next` to guarantee ascending chain termination.
    ///
    /// **Required law**: `self ⊑ widen(self, next)` ∧ `next ⊑ widen(self, next)`.
    /// The ascending chain `x₀, widen(x₀, x₁), ...` must stabilize in finite steps.
    fn widen(&self, next: &Self) -> Self;

    /// Narrow `self` with `next` to refine a post-fixpoint downward.
    ///
    /// **Required law**: `self ⊓ next ⊑ narrow(self, next) ⊑ self`.
    /// The descending chain must stabilize in finite steps.
    ///
    /// Default: no refinement (returns `self`). Domains that do not need
    /// narrowing (or where `widen` already produces precise results) can
    /// leave this as-is.
    fn narrow(&self, _next: &Self) -> Self
    where
        Self: Clone,
    {
        self.clone()
    }
}
```

There is no blanket implementation. For simple domains where `widen = join` is
correct, the implementor writes one line:

```rust
impl AbstractValue for MySmallLattice {
    fn widen(&self, next: &Self) -> Self {
        self.join(next)
    }
    // narrow uses the default no-op
}
```

`AbstractInterpreter` extends `Interpreter` — an abstract interpreter IS an
interpreter with additional worklist scheduling. This means `Session<'ir, A, S>`
where `A: AbstractInterpreter` works naturally; no separate session type is
needed for abstract execution. Two concrete analysis engines are built on top:
dense forward analysis and sparse forward single-SSA analysis.

```rust
pub trait AbstractInterpreter: Interpreter
where
    Self::Value: AbstractValue,
{
    fn enqueue(&mut self, block: Block) -> Result<(), Self::Error>;
    fn dequeue(&mut self) -> Result<Option<Block>, Self::Error>;
    fn notify_branch(&mut self, from: Statement, targets: &[Block]) -> Result<(), Self::Error>;
}
```

A typical abstract interpreter wraps a `StackInterpreter<V, E, G>` and
delegates `Interpreter` methods to it. Abstract interpreters may need global
state just as concrete interpreters do. For example, a virtual address
allocator for alias analysis must persist across call frames (function A
allocates address `0x100`, function B must allocate `0x200`). Such state does
not fit in any single `Frame<V>` and is not part of the abstract value type `V`
itself. The `Global` type parameter on `StackInterpreter` threads this state
through so dialect `interpret` impls can access it via `interpreter.global()`.
Analyses that need no global state use `G = ()`.

Dense forward analysis propagates facts at all reachable program points until
fixpoint.

```rust
pub struct DenseForwardAnalysis<V: AbstractValue, E> {
    /* worklist + lattice store + interpreter base */
}

impl<V: AbstractValue, E> DenseForwardAnalysis<V, E> {
    pub fn enqueue_program_point(&mut self, stmt: Statement) -> Result<(), E> { /* ... */ }
    pub fn run_to_fixpoint(&mut self) -> Result<(), E> { /* ... */ }
}
```

Sparse forward analysis tracks only one seeded SSA value (or a small seed set)
and follows uses along that value's dataflow graph. It discovers use-sites via
the existing `SSAInfo<L>::uses()` API (`crates/kirin-ir/src/node/ssa.rs`),
which returns `&HashSet<Use>` where each `Use` records the consuming statement
and operand index.

```rust
pub struct SparseForwardAnalysis<V: AbstractValue, E> {
    /* seed set + use-def frontier + lattice store + interpreter base */
}

impl<V: AbstractValue, E> SparseForwardAnalysis<V, E> {
    pub fn seed_value(&mut self, value: SSAValue) -> Result<(), E> { /* ... */ }
    pub fn seed_values(&mut self, values: impl IntoIterator<Item = SSAValue>) -> Result<(), E> { /* ... */ }
    pub fn enqueue_use_site(&mut self, stmt: Statement) -> Result<(), E> { /* ... */ }
    pub fn run_sparse(&mut self) -> Result<(), E> { /* ... */ }
}
```

`ExecutionControl` is the handoff protocol from dialect semantics back to the
session driver. `Jump` and `Fork` carry values for target block arguments,
matching the IR's `BlockInfo::arguments: Vec<BlockArgument>`
(`crates/kirin-ir/src/node/block.rs`). Functions return a single value,
consistent with `Signature<T> { ret: T }` (`crates/kirin-ir/src/signature.rs`).
Kirin does not impose a type system — dialects parameterize types, so
downstream developers who need multi-return without tuple types can encode it
in their value domain `V` (e.g., `V::Tuple(Vec<...>)`).
`Break` suspends execution at the current statement, allowing external callers
to inspect interpreter state and resume later.

**`Fork` operational semantics**: when a dialect `interpret` returns
`Fork([(b₁, v̄₁), ..., (bₙ, v̄ₙ)])`, the `AbstractInterpreter` enqueues
all target blocks into its worklist. For each target `bᵢ`, the block
arguments are bound to `v̄ᵢ`. On revisit, the abstract interpreter merges
(joins) the incoming abstract state with the existing state at the block
entry using `Lattice::join`. If the merged state exceeds a widening
threshold, `AbstractValue::widen` is applied. This is the standard worklist
algorithm: `Fork` is the abstract counterpart of `Jump` for
non-deterministic branching.

```rust
pub enum ExecutionControl<V> {
    Continue,
    Jump(Block, Vec<V>),
    Fork(Vec<(Block, Vec<V>)>),
    Call { callee: SpecializedFunction, args: Vec<V> },
    Return(V),
    Break,
    Halt,
}
```

`Session` is the single execution driver. It is generic over `I: Interpreter`,
so the same session type works for both concrete interpreters
(`StackInterpreter<V, E, G>`) and abstract interpreters (any type implementing
`Interpreter` + `AbstractInterpreter`). It holds a reference to the full
`Pipeline<S>`, tracks the active stage, and provides all execution APIs. The
statement cursor lives in `Frame<V>` (accessed via the interpreter's current
frame), not in the session. Having the full pipeline available is essential
because call conventions may need to resolve abstract functions to concrete
`SpecializedFunction` targets — a lookup that requires pipeline-level function
tables.

The `step` method returns the raw `ExecutionControl` from dialect semantics
without applying cursor mutations. The separate `advance` method applies
cursor mutations for a given control action. This two-method split lets
callers inspect control flow before committing to it (useful for debuggers
and analysis tools). The `run` and `run_until_break` convenience methods
combine both in a loop.

```rust
pub struct Session<'ir, I, S>
where
    I: Interpreter,
    S: CompileStageInfo,
{
    interpreter: I,
    pipeline: &'ir Pipeline<S>,
    active_stage: CompileStage,
    breakpoints: HashSet<Statement>,
}

impl<'ir, I, S> Session<'ir, I, S>
where
    I: Interpreter,
    S: CompileStageInfo,
{
    pub fn new(
        interpreter: I,
        pipeline: &'ir Pipeline<S>,
        active_stage: CompileStage,
    ) -> Self {
        Self {
            interpreter,
            pipeline,
            active_stage,
            breakpoints: HashSet::default(),
        }
    }

    /// Returns the pipeline reference with lifetime `'ir` (independent of `&self`).
    pub fn pipeline(&self) -> &'ir Pipeline<S> { self.pipeline }

    /// Returns a mutable reference to the interpreter.
    pub fn interpreter_mut(&mut self) -> &mut I {
        &mut self.interpreter
    }

    pub fn set_breakpoints(&mut self, stmts: HashSet<Statement>) { /* ... */ }
    pub fn clear_breakpoints(&mut self) { /* ... */ }

    /// Execute the current statement's dialect semantics.
    /// Returns the raw `ExecutionControl` without advancing the cursor.
    ///
    /// Dispatch: reads cursor from the current frame, resolves the active
    /// stage's `StageInfo<L>` from the pipeline, retrieves
    /// `stmt.definition(stage)` to get `&L`, then calls
    /// `L::interpret(&mut self.interpreter)`.
    pub fn step<L>(&mut self) -> Result<ExecutionControl<I::Value>, I::Error>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<I>,
    {
        let stage: &StageInfo<L> = /* resolve from pipeline + active_stage */;
        let cursor = self.interpreter.current_frame()?.cursor()
            .expect("no current statement");
        let def: &L = cursor.definition(stage);
        def.interpret(&mut self.interpreter)
    }

    /// Apply cursor mutations for a control action.
    /// - `Continue`: advance cursor to next statement in current block.
    /// - `Jump(block, args)`: bind args to block arguments, set cursor to
    ///   block entry.
    /// - `Call { callee, args }`: resolve callee from the pipeline, push a
    ///   new frame with cursor set to the callee's entry point.
    /// - `Return(value)`: pop the current frame, restore caller cursor.
    /// - `Break`, `Halt`: no cursor change.
    pub fn advance<L>(
        &mut self,
        control: &ExecutionControl<I::Value>,
    ) -> Result<(), I::Error>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let stage: &StageInfo<L> = /* resolve from pipeline + active_stage */;
        match control {
            ExecutionControl::Continue => { /* advance cursor to next stmt */ }
            ExecutionControl::Jump(block, args) => { /* bind args, set cursor */ }
            ExecutionControl::Call { callee, args } => {
                /* resolve callee from pipeline, push frame */
            }
            ExecutionControl::Return(value) => {
                /* pop frame, restore caller cursor */
            }
            _ => { /* no cursor change */ }
        }
        Ok(())
    }

    /// Run statements until Return, Halt, or a cross-frame event (Call).
    /// Ignores breakpoints. Returns the control action that ended execution.
    pub fn run<L>(&mut self) -> Result<ExecutionControl<I::Value>, I::Error>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<I>,
    {
        loop {
            let control = self.step::<L>()?;
            match &control {
                ExecutionControl::Continue | ExecutionControl::Jump(..) => {
                    self.advance::<L>(&control)?;
                }
                ExecutionControl::Break => {
                    // Ignore Break from dialect intrinsics in run mode.
                    self.advance::<L>(&ExecutionControl::Continue)?;
                }
                _ => return Ok(control),
            }
        }
    }

    /// Run statements until a breakpoint, Return, Halt, or a cross-frame
    /// event (Call). Returns the control action that caused suspension.
    pub fn run_until_break<L>(&mut self) -> Result<ExecutionControl<I::Value>, I::Error>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<I>,
    {
        loop {
            if let Some(cursor) = self.interpreter.current_frame()?.cursor() {
                if self.breakpoints.contains(&cursor) {
                    return Ok(ExecutionControl::Break);
                }
            }
            let control = self.step::<L>()?;
            match &control {
                ExecutionControl::Continue | ExecutionControl::Jump(..) => {
                    self.advance::<L>(&control)?;
                }
                _ => return Ok(control),
            }
        }
    }

    /// Execute a call to an already-resolved specialized function.
    /// Pushes a new frame, runs to completion, pops the frame, and returns
    /// the single result value consistent with `Signature { ret: T }`.
    pub fn call<L>(
        &mut self,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<I::Value, I::Error>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<I>,
    {
        /* push frame, run to Return/Halt, pop frame, return value */
    }
}
```

Typical flow:

1. Build or load `Pipeline<S>` and choose an active `CompileStage`.
2. Construct a `StackInterpreter<V, E, G>` (or custom `Interpreter` impl).
3. Create a `Session` wrapping the pipeline reference and interpreter.
4. Call `session.step::<L>()` to execute the current statement. The step
   reads the cursor from the current frame, resolves `StageInfo<L>` from the
   pipeline, dispatches via `stmt.definition(stage).interpret(interpreter)`,
   and returns the raw `ExecutionControl`.
5. Inspect the control action, then call `session.advance::<L>(&control)` to
   apply cursor mutations:
   - `Continue`: advance cursor to next statement.
   - `Jump(block, args)`: bind `args` to the target block's `BlockArgument`
     SSA values and set cursor to block entry.
   - `Call { callee, args }`: resolve callee from the pipeline, push a new
     frame.
   - `Return(value)`: pop frame, restore caller cursor.
6. Continue until `ExecutionControl::Halt`.
7. Alternatively, use `session.run::<L>()` to run to completion ignoring
   breakpoints, or `session.run_until_break::<L>()` to stop at breakpoints.

## Reference-level Explanation

### API and syntax changes

- Add a new crate: `crates/kirin-interpreter`.
- New public APIs in `kirin-interpreter`:
  - `Frame<V>`
  - `Interpreter` (state contract: frame/value access + call-frame transitions)
  - `StackInterpreter<V, E, G = ()>`
  - `Interpretable<I>` (dialect execution hook)
  - `AbstractValue` (extends `Lattice` with `widen` + default `narrow`)
  - `AbstractInterpreter`
  - `DenseForwardAnalysis<V, E>`
  - `SparseForwardAnalysis<V, E>`
  - `ExecutionControl<V>` (control protocol used by dialect `interpret`)
  - `Session<'ir, I, S>` where `I: Interpreter` (with `step`, `advance`,
    `run`, `run_until_break`, `call`)
- Dependencies: use `fxhash` crate (`FxHashMap`) for frame value storage;
  depend on `kirin-ir` for `Lattice`, `SSAValue`, `Block`, etc.
- Optional re-export from top-level `kirin` crate for ergonomic use.
- No syntax changes to parser/printer text format.

### Semantics and invariants

- `Frame<V>` is the shared frame representation for call execution and can be
  reused directly by downstream interpreter implementations.
- `Frame<V>` provides native `read(SSAValue)` / `write(ResultValue, V)` helpers
  over frame-local value storage.
- `Frame<V>::values` uses `fxhash::FxHashMap<SSAValue, V>` for sparse per-frame
  storage. SSA value IDs are allocated from a stage-wide arena
  (`StageInfo<L>::ssas`), so they are dense across the full stage but sparse
  within any single function frame. `FxHashMap` avoids allocating memory
  proportional to the stage arena size.
- Constructor shape is intentionally minimal:
  `Frame::new(callee, cursor)`. A builder is optional and should only wrap
  these same required fields.
- `Interpreter` is a **state contract only**. It defines frame access
  (`current_frame`, `current_frame_mut`), SSA read/write, and call-frame
  transitions (`push_call_frame`, `pop_call_frame`). It does **not** define
  execution entrypoints — those belong to `Session::step` and
  `Session::call`.
- `StackInterpreter` is the default `Interpreter` implementation and owns the
  call stack with `current_frame` / `current_frame_mut` accessors.
- `StackInterpreter` also owns optional process-global runtime state `G` (for
  example IO buffers or virtual screen state). `G` defaults to `()`, so users
  that do not need global state can use `StackInterpreter::new(...)` without
  providing extra inputs.
- `Interpreter::read_ref(SSAValue)` returns `&Value` without cloning — for
  inspection, debugging, and cases where ownership is not needed.
  `Interpreter::read(SSAValue)` returns a cloned `Value` (requires
  `Value: Clone`) — preferred in dialect `interpret` impls where the caller
  needs ownership. Both return `Err(E)` if no value is bound.
- `Interpreter::write(ResultValue, Value)` writes exactly one result binding.
  Statements with multiple results must call `write` for each result.
- `push_call_frame(frame)` enters callee execution context using a
  framework-defined `Frame<V>`.
- `pop_call_frame()` returns the popped `Frame<V>` and restores caller context.
- Interpreter implementations maintain a stack of `Frame<V>` values; pushing
  and popping call frames updates this stack.
- **Dialect dispatch**: `Session::step::<L>()` resolves `StageInfo<L>` from
  the pipeline via the active stage, reads the cursor from the current frame,
  retrieves `stmt.definition(stage)` (returning `&L`), then calls
  `L::interpret(&mut self.interpreter)`. This is static dispatch through the
  dialect enum type `L`; no runtime registry or trait object table is needed.
- **Block argument passing**: `ExecutionControl::Jump(Block, Vec<V>)` carries
  values for the target block's arguments. `Session::advance` binds these
  values to the block's `BlockArgument` SSA values
  (`BlockInfo::arguments: Vec<BlockArgument>`,
  `crates/kirin-ir/src/node/block.rs`) before setting the cursor.
  `Fork(Vec<(Block, Vec<V>)>)` similarly carries per-target argument values
  for abstract interpretation.
- **Borrow ergonomics**: `Session::pipeline()` returns `&'ir Pipeline<S>`
  with a lifetime independent of `&self`, so callers can extract a stage
  reference and then call `interpreter_mut()` without borrow conflicts.
- **Cursor ownership**: `Frame<V>` is the sole owner of the statement cursor.
  The cursor tracks which statement is next in the current activation. When a
  call frame is pushed, the current cursor is saved in the caller's frame;
  when popped, it is restored. `Session` does not maintain its own cursor —
  it reads from the interpreter's current frame.
- **Step/advance split**: `Session::step` returns the raw `ExecutionControl`
  from dialect semantics without applying cursor mutations.
  `Session::advance` applies cursor mutations for a given control action
  (`Continue` advances cursor, `Jump` binds block args and sets cursor,
  `Call` resolves callee and pushes a frame, `Return` pops a frame). This
  split lets callers inspect control flow before committing (useful for
  debuggers and analysis tools). `run` and `run_until_break` combine both
  in a loop.
  **Contract**: `advance` must be called with the `ExecutionControl` value
  returned by the immediately preceding `step` on the same session.
  Calling `advance` with a stale or fabricated control value is a logic
  error and may corrupt the cursor or call stack. This contract is
  documented but not enforced at the type level — callers (debuggers,
  analysis tools) are expected to uphold it.
- **Error recovery**: If a dialect `interpret` returns `Err(e)`, the
  interpreter state remains valid at the current frame. The caller can inspect
  state, fix the issue, and retry the same statement or manually unwind frames.
  Errors do not automatically unwind the call stack or poison the session.
- **Function resolution**: `Session::call` takes an already-resolved
  `SpecializedFunction`. Function resolution (compile-time dispatch via
  `SignatureSemantics` or runtime lookup) is the responsibility of
  dialect-specific call statement implementations, not the interpreter
  framework. The session holds the full `Pipeline<S>`, making function lookup
  available to dialect impls that need it.
- `Session<'ir, I, S>` carries `&'ir Pipeline<S>` plus `active_stage` for
  stage selection, the interpreter `I` for execution state, and optional
  breakpoints. It is the single execution driver — there is no separate
  stage-local session type. Being generic over `I: Interpreter`, it works
  with both `StackInterpreter` (concrete) and `AbstractInterpreter` impls.
- `ExecutionControl` defines interpreter-driven control transfer:
  - `Continue`: advance to next statement in current block.
  - `Jump(Block, Vec<V>)`: set cursor to target block entry and bind
    argument values to the block's `BlockArgument` SSA values.
  - `Fork(Vec<(Block, Vec<V>)>)`: branch into multiple targets with
    per-target argument values; used **exclusively by abstract
    interpretation** to explore/merge control flows. Semantics: the
    abstract interpreter enqueues all targets into its worklist, binds
    block arguments per target, and merges (joins) incoming state on
    revisit. Widening is applied when the merged state exceeds a
    convergence threshold. **`Fork` is not for async/concurrent
    execution.** `Fork` is an epistemic statement ("this branch could go
    either way — analyze both and merge"), not an operational one ("spawn
    concurrent tasks"). Async execution requires different primitives
    (task spawn, yield, await) with different state management
    (multi-stack schedulers, shared-state coordination) — see Future
    Possibilities.
  - `Call { callee, args }`: call a concrete `SpecializedFunction`; the
    session creates a `Frame<V>` and invokes `push_call_frame(frame)`.
  - `Return(V)`: finish current frame with a single return value (consistent
    with `Signature { ret: T }`) and request `pop_call_frame()`.
  - `Break`: suspend execution at the current statement without advancing the
    cursor. The session driver returns control to the caller, who can inspect
    interpreter state and call `step` again to resume. `Break` can originate
    from the session's breakpoint set (checked before each step) or from a
    dialect `interpret` impl directly (e.g., debugging intrinsics).
  - `Halt`: terminate the session without further steps.
- `AbstractValue` extends the existing `Lattice` trait
  (`crates/kirin-ir/src/lattice.rs`) with a required `widen` method and a
  default no-op `narrow` method. There is no blanket implementation — every
  abstract value type must explicitly define widening. Even finite lattices
  may need widening distinct from `join` when practical lattice height is too
  large for naive fixpoint convergence. Algebraic contracts (`widen`:
  `x ⊑ widen(x, y) ∧ y ⊑ widen(x, y)`, ascending chain stabilizes;
  `narrow`: `x ⊓ y ⊑ narrow(x, y) ⊑ x`, descending chain stabilizes) are
  documented on the trait and verified by a property-testing harness in
  `kirin-test-utils`.
- `AbstractInterpreter` extends `Interpreter` with a worklist scheduling API
  (`enqueue`/`dequeue`/`notify_branch`). Because it is an `Interpreter`,
  `Session<'ir, A, S>` works directly for abstract execution.
- `DenseForwardAnalysis` propagates facts across all reachable
  statements/blocks and computes global forward fixpoints.
- `SparseForwardAnalysis` propagates facts only from seeded SSA values to their
  use sites. It discovers use-sites via the existing `SSAInfo<L>::uses()` API
  (`crates/kirin-ir/src/node/ssa.rs`), which returns `&HashSet<Use>` where
  each `Use { stmt, operand_index }` identifies a consuming statement.
- Both abstract modes share the same base `AbstractInterpreter` state contract
  and `AbstractValue` merge/widen behavior.

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-interpreter` (new) | `Interpreter` (state contract), `StackInterpreter<V, E, G = ()>`, shared `FxHashMap`-backed `Frame<V>`, `Interpretable<I>` dispatch, `Session::step` / `Session::call`, `AbstractValue` / `AbstractInterpreter`, dense+sparse abstract analysis engines | unit tests for frame stack, frame read/write behavior, dialect dispatch via `stmt.definition(stage)`, specialized-call control, cursor stepping, block argument binding, breakpoint handling, dense fixpoint scheduling, sparse seed propagation via `SSAInfo::uses()`, global-state access |
| `kirin-ir` | integration touch points for `Pipeline`, `SpecializedFunction`, `StageInfo`, `Statement`, `SSAValue`, `ResultValue` usage and docs | integration tests that execute specialized-call paths end to end |
| `kirin-function` / `kirin-cf` / `kirin-scf` | implement `Interpretable<I>` for core control/call operations | per-dialect interpretation tests and end-to-end function execution tests |
| `kirin-test-utils` | shared fixtures and mock interpreters used across crates | add reusable interpreter harness helpers used by multiple crate test suites |

## Drawbacks

- Adds a new public API surface that must remain stable.
- Requires dialect crates to implement and maintain execution semantics.
- The single `Session` type carries both pipeline-level and stage-level
  concerns, which increases its surface area.

## Rationale and Alternatives

### Proposed approach rationale

- Keeps runtime responsibilities minimal and composable:
  IR stays in `kirin-ir`; execution policy is in a dedicated crate.
- Keeps concrete interpretation simple with one default stack interpreter, and
  allows abstract interpretation as an explicit layer on top.
- Uses existing Kirin terminology and IDs (`SSAValue`, `ResultValue`,
  `Function`, `StagedFunction`, `SpecializedFunction`, `Pipeline`,
  `StageInfo`) rather than introducing duplicate runtime representations.

### Alternative A

- Description: Put interpreter traits directly in `kirin-ir`.
- Pros: fewer crates, direct access to IR internals.
- Cons: mixes execution policy with IR core data model, increases API coupling,
  and makes `kirin-ir` harder to keep minimal.
- Reason not chosen: this RFC prefers separation of concerns and cleaner crate
  boundaries.

### Alternative B

- Description: Skip a shared trait model and let each dialect/runtime define its
  own interpreter API.
- Pros: maximum local flexibility.
- Cons: duplicated abstractions, no ecosystem-level interoperability, harder to
  reuse tests/utilities.
- Reason not chosen: Kirin needs common execution contracts across dialects.

## Prior Art

- Abstract interpretation frameworks (Cousot & Cousot '77, '79) that separate
  transfer functions from the fixed-point engine and require join/widen/narrow
  operators with algebraic contracts for termination and soundness.
- MLIR's dataflow analysis framework (`mlir/include/mlir/Analysis/DataFlowAnalysis.h`):
  separates `AbstractSparseLattice` (lattice element with `join`/meet) from
  `DataFlowAnalysis` (the fixed-point engine). Analyses subclass
  `DataFlowAnalysis` and override `visitOperation` — analogous to this RFC's
  `Interpretable<I>::interpret`. MLIR provides both dense
  (`ForwardDataFlowAnalysis`) and sparse (`AbstractSparseForwardDataFlowAnalysis`)
  engines, which directly inspired this RFC's `DenseForwardAnalysis` and
  `SparseForwardAnalysis` split. Key difference: MLIR bakes in MLIR-specific
  IR types (`Operation *`, `Value`, `Block *`), while this RFC is generic over
  Kirin's `StageInfo<L>`-parameterized IR.
- Rust trait-based evaluator designs that keep value/error domains generic
  (e.g., cranelift's `InstBuilder` pattern of separating IR definition from
  execution policy).

## Backward Compatibility and Migration

- Breaking changes: none (additive RFC).
- Migration steps:
  1. Introduce `kirin-interpreter` crate and baseline traits/types.
  2. Add `Interpretable<I>` impls to selected dialect crates.
  3. Add integration tests using shared `kirin-test-utils` harnesses.
- Compatibility strategy: existing parser/printer and IR builders remain
  unchanged; interpreters can be adopted incrementally per dialect.

## How to Teach This

- Add a design doc (`design/interpreter.md`) that mirrors this RFC with a
  small executable walkthrough.
- Add examples in `example/` showing:
  - one concrete interpreter (e.g., arithmetic evaluator)
  - one dense forward abstract interpreter (interval/lattice domain)
  - one sparse forward abstract interpreter seeded from a chosen `SSAValue`
- Document implementation checklist for dialect maintainers:
  define `interpret` behavior for each operation and test control flow/calls.

## Reference Implementation Plan

1. Add `crates/kirin-interpreter` with trait definitions, shared
   `FxHashMap`-backed `Frame<V>`, and `Session` skeleton plus unit tests.
2. Implement `Session::step`, `Session::advance`, and specialized
   call-frame handling (`Session::call`).
3. Implement `Interpretable<I>` for a minimal dialect set (`kirin-constant`,
   arithmetic ops, `ret`, branch/cf as needed for end-to-end tests).
4. Implement dense forward abstract interpreter flow (`run_to_fixpoint`) for a
   reference domain.
5. Implement sparse forward abstract interpreter flow (`seed_value` + `run_sparse`).
6. Add shared mock interpreter/test harness in `kirin-test-utils`.
7. Add documentation and example programs.

### Acceptance Criteria

- [ ] `kirin-interpreter` exposes the core API items listed in this RFC.
- [ ] `Interpreter` trait is state-only (no `execute_statement`/`call`);
  execution lives in `Session::step` and `Session::call`.
- [ ] `Session::step` dispatches via `stmt.definition(stage).interpret()`
  with static dispatch through the dialect enum `L`.
- [ ] At least one concrete interpreter can execute a staged function end to
  end, including call-frame push/pop and block argument binding.
- [ ] At least one interpreter implementation reuses framework `Frame<V>`
  directly (without redefining an equivalent frame type).
- [ ] `Frame<V>` uses `fxhash::FxHashMap` for sparse frame storage in the
  reference implementation.
- [ ] Call control executes `SpecializedFunction` targets end to end;
  `Session::call` returns a single value matching `Signature { ret: T }`.
- [ ] `ExecutionControl::Jump` carries block argument values and the session
  binds them to `BlockArgument` SSA values.
- [ ] At least one dense forward abstract interpreter demonstrates `join`,
  `widen`, and `narrow` (via `AbstractValue` trait) through looped control
  flow, showing that narrowing refines widened results.
- [ ] At least one sparse forward abstract interpreter demonstrates seeded
  propagation from a chosen `SSAValue` to its dataflow uses via
  `SSAInfo::uses()`.
- [ ] `kirin-test-utils` contains a property-testing harness that verifies
  `AbstractValue` algebraic contracts: widening monotonicity
  (`x ⊑ widen(x, y) ∧ y ⊑ widen(x, y)`), ascending chain stabilization,
  narrowing bounds (`x ⊓ y ⊑ narrow(x, y) ⊑ x`), and descending chain
  stabilization for user-defined abstract value types.
- [ ] `kirin-test-utils` contains reusable interpreter test helpers used by
  multiple crates.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - per-dialect `Interpretable<I>` rollout checklist
  - performance benchmarking of session stepping and frame handling

## Unresolved Questions

- Should `Session` own `I` (as shown) or borrow `&mut I` to simplify
  embedding in larger runtimes?
- **Frame storage optimization**: `FxHashMap` is correct and simple for v1,
  but most function frames touch only 5–50 SSA values where hash map overhead
  (heap-allocated bucket array, hash probing) is measurably slower than
  linear scan or inline storage. Future work should profile and consider
  tiered storage (e.g., `SmallVec<[(SSAValue, V); 16]>` for small frames,
  falling back to `FxHashMap` for large ones). The `Frame<V>` type does not
  expose its storage backend in the public API, so this change is
  non-breaking.
- **`Vec<V>` in `ExecutionControl`**: `Jump(Block, Vec<V>)`,
  `Call { args: Vec<V> }`, and `Fork` allocate on every step. For tight
  interpretation loops this may show up in profiles. Future work should
  consider `SmallVec<[V; 4]>` (most blocks have ≤4 arguments) or a
  pre-allocated scratch buffer. This is an internal change to
  `ExecutionControl` and does not affect the `Interpretable` trait signature.

## Future Possibilities

- Parallel or worklist-based interpreter sessions for dataflow analyses.
- Debugger hooks beyond basic breakpoints (stepping callbacks, trace collection,
  conditional breakpoints, watchpoints).
- **Host-level async**: an `AsyncInterpretable<I>` trait where `interpret` is
  `async fn`, allowing dialect impls to perform host IO (network, filesystem)
  without blocking the thread. The `Interpreter` trait itself stays
  synchronous (it is pure state); only the dialect hook and `Session` step/run
  methods gain async variants. This is orthogonal to IR-level concurrency.
- **IR-level concurrency**: new `ExecutionControl` variants (`Spawn`,
  `Yield`, `Await`) for modeling concurrent tasks in the IR. This requires a
  multi-stack interpreter (one call stack per task), a task scheduler, and a
  shared-state coordination model. `Fork` is **not** suitable for this — it
  models abstract non-determinism (analyze all branches and merge), not
  operational concurrency (spawn tasks that run and communicate). IR-level
  concurrency is a separate RFC once the base interpreter framework is stable.

## Checklist Status

- [x] State the problem and motivation concretely.
- [x] Define clear goals and non-goals.
- [x] Describe current behavior with exact file references.
- [x] Describe proposed behavior with enough detail to implement.
- [x] Identify all affected crates and likely touch points.
- [x] Include at least two alternatives with trade-offs.
- [x] Explain backward compatibility and migration impact.
- [x] Define test and validation work per affected crate.
- [x] List key risks and mitigations.
- [x] End with explicit open questions or decision points.
- [x] Keep terminology consistent with Kirin docs and code.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-12T16:09:01.905258Z | RFC created from template |
| 2026-02-12 | Filled RFC content for interpreter framework traits, semantics, crate impacts, alternatives, and validation plan |
| 2026-02-12 | Revised: (1) split Interpreter to state-only, move execution to sessions; (2) specify dialect dispatch via stmt.definition(stage); (3) replace AbstractDomain with AbstractValue: Lattice + widen, no blanket impl; (4) Jump/Fork carry block argument values; (5) improve FxHashMap justification (stage-dense, function-sparse); (6) singular return matching Signature { ret: T }; (7) reference existing SSAInfo::uses() for sparse analysis; (8) explicit stage-ref parameter for borrow ergonomics |
| 2026-02-12 | Added ExecutionControl::Break and Session breakpoint mode (set_breakpoints, run_until_break) |
| 2026-02-12 | Revised: (1) cursor lives in Frame<V> only, removed from sessions; (2) step/advance two-method split for inspect-then-commit; (3) AbstractInterpreter gets Global type parameter; (4) call takes already-resolved SpecializedFunction, resolution is dialect responsibility; (5) Session struct reordered with breakpoints field; (6) errors are recoverable, state valid at current frame |
| 2026-02-12 | Restructured: removed StageSession and PipelineSession, consolidated into single `Session` that owns `StackInterpreter` + `&'ir Pipeline<S>`. All execution APIs (step, advance, run, run_until_break, call) live on Session. Motivation: call conventions may need to resolve abstract functions, requiring pipeline-level function lookup at the execution driver level. |
| 2026-02-12 | Generalized: Session is now `Session<'ir, I, S>` generic over `I: Interpreter` instead of hardcoding `StackInterpreter`. AbstractInterpreter now extends Interpreter (supertrait) instead of exposing `base()`/`base_mut()` accessors to a hardcoded StackInterpreter. Added `Session::new` constructor. |
| 2026-02-12 | Review-driven revisions: (1) `Interpretable<I>` keeps `I` parameterization with trait-bounded dispatch pattern — pure ops bound on `I::Value`, effectful ops bound on `I` directly; (2) added `read_ref` returning `&V` alongside cloning `read` for inspection/debugging; (3) `AbstractValue` gains default no-op `narrow` method and documented algebraic laws for `widen`/`narrow`; (4) `Fork` gets explicit operational semantics (enqueue all targets, merge on revisit, widen at threshold); (5) step/advance contract documented (advance must use control from preceding step); (6) `Return(V)` kept — Kirin is type-system-agnostic, multi-return encoded in `V` by downstream; (7) open questions added for `FxHashMap` frame storage optimization and `SmallVec` for `ExecutionControl`; (8) acceptance criteria adds property-testing harness for `AbstractValue` algebraic contracts. |
| 2026-02-12 | Clarified `Fork` is exclusively for abstract interpretation, not async/concurrent execution. Added MLIR dataflow analysis framework to Prior Art. Expanded Future Possibilities with host-level async (`AsyncInterpretable`) and IR-level concurrency (`Spawn`/`Yield`/`Await`) as separate future work. |
