# Interpreter Frame Fusion

**Date:** 2026-04-27
**Status:** design draft

## Summary

This design fuses the old split between call frames and cursors into one
generic frame stack. A frame is a continuation object anchored at an IR
traversal location. Some frames are standard IR traversal frames supplied by
the interpreter crate, such as block, region, statement, and function frames.
Other frames are defined by dialects, such as an `scf.for` frame.

The interpreter shell is generic over the total frame, completion, and error
types. It owns the frame stack, the immutable program root, and the SSA
environment stack. It does not understand dialect control flow. Frames return
small stack-shape effects, and they mutate SSA state through constrained
interpreter capabilities.

The design keeps four channels explicit:

1. `Location` describes semantic-independent IR traversal positions.
2. `Frame` carries semantic traversal state.
3. `Completion` reports that a frame has completed.
4. `Error` reports protocol, IR, or dialect failures.

`Frame`, `Completion`, and `Error` all use the lift/project algebra. Failed
projection for bubbling paths returns the original value.

## Goals

- Generalize cursors and call frames into one frame stack.
- Keep the interpreter shell generic over the total frame type.
- Let dialect authors define frames and completions without changing the
  interpreter crate.
- Provide reusable standard frames for common IR traversal and call conventions.
- Keep SSA activation storage owned by the interpreter shell, not by dialect
  frames.
- Keep the driver loop flat and deterministic.
- Keep `FrameEffect` specific to frame-stack transitions.

## Non-Goals

- The interpreter does not mutate the IR program.
- The initial shell is not a scheduler or worklist engine.
- The first version does not generate composition glue. Macros can reduce
  boilerplate later.
- `Location` starts in the new interpreter crate, not `kirin-ir`. It may move
  to `kirin-ir` later if the abstraction stabilizes.

## Location

`Location` is IR-specific and not generic. It describes actual traversal
positions, not dialect semantic phases.

```rust
pub struct Location {
    pub stage: CompileStage,
    pub position: Position,
}

pub enum Traversal<T> {
    Entry,
    Active(T),
    Exit,
}

pub enum Position {
    Function {
        function: Function,
        traversal: Traversal<StagedFunction>,
    },
    StagedFunction {
        function: StagedFunction,
        traversal: Traversal<SpecializedFunction>,
    },
    SpecializedFunction {
        function: SpecializedFunction,
        traversal: Traversal<Statement>,
    },
    Region {
        region: Region,
        traversal: Traversal<Block>,
    },
    Block {
        block: Block,
        traversal: Traversal<Statement>,
    },
    DiGraph {
        graph: DiGraph,
        traversal: Traversal<Statement>,
    },
    UnGraph {
        graph: UnGraph,
        traversal: Traversal<Statement>,
    },
}
```

There is no standalone statement location. A statement is always the active
child of a traversal location, for example:

```rust
Position::Block {
    block,
    traversal: Traversal::Active(statement),
}
```

`Traversal::Active(child)` always means the parent traversal is focused on that
child until the child completes. For example, a block focused on an `scf.for`
statement remains at the active statement location while the loop frame and
body block frames run underneath it.

Semantic phases like `ForCondition`, `WaitingForYield`, or `DispatchingCall`
do not belong in `Location`. They belong in frame state.

## Env Stack

SSA activation storage is owned by the interpreter shell. Frames that need an
activation store carry an `EnvIndex`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnvIndex(usize);

pub trait Env<V> {
    type Error;

    fn push(&mut self) -> EnvIndex;
    fn pop(&mut self) -> Result<(), Self::Error>;
    fn current(&self) -> Result<EnvIndex, Self::Error>;

    fn read(&self, env: EnvIndex, value: SSAValue) -> Result<V, Self::Error>;
    fn write(&mut self, env: EnvIndex, value: SSAValue, data: V)
        -> Result<(), Self::Error>;
}
```

`EnvIndex` is an opaque stack slot index. A plain index is enough for now; it
does not need a generation counter. The standard env implementation validates
that an index is live by checking it against the current stack length.

`pop` only removes the top environment. `read` and `write` take an explicit
`EnvIndex`, and writes are allowed to any live environment index. This lets a
parent frame resume and continue writing to its own activation after a child
call has pushed and popped another activation.

## FrameEffect

`FrameEffect` is only the frame-stack protocol. It does not contain env operations,
SSA writes, or pipeline access commands.

```rust
pub enum FrameEffect<F, C> {
    Continue(F),
    Push { parent: F, child: F },
    Complete(C),
}
```

Env mutation happens through `&mut I` capability traits on the interpreter.
This keeps the returned effect small and avoids turning `FrameEffect` into a command
language.

`Push` pushes exactly one child. If a frame needs to enter several layers, it
does so over multiple driver ticks. This keeps each transition easy to trace:

```text
Continue(f)             => push f
Push { parent, child }  => push parent, then child
Complete(c), parent     => resume parent with c
Complete(c), root       => final completion
```

`Continue(f)` means the same frame remains on top and runs on the next driver
tick. The first shell is a deterministic stack machine, not a scheduler.

Both `step` and `resume` may return any `FrameEffect`, including `Push`. This is
needed for frames such as `scf.for`, where receiving a body yield may
immediately push the next loop-body block frame.

## Frame

Frames are consumed by value during stepping. This avoids a simultaneous
`&mut frame` and `&mut interpreter` borrow.

```rust
pub trait Frame<I, F, C, E>: Sized {
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E>;

    fn resume(
        self,
        completion: C,
        interp: &mut I,
    ) -> Result<FrameEffect<F, C>, E>;
}
```

The trait is generic over the total interpreter, frame, completion, and error
types. Standard frames implement it with bounds that say which local pieces can
be lifted into the total types and which completions can be projected out of
the total completion.

The shell uses the closed universe:

```rust
F: Frame<Interpreter<'ir, S, F, C, E, V>, F, C, E>
```

Reusable frame components, such as `BlockFrame<V>` and `ScfForFrame<V>`, are
written against arbitrary total `F`, `C`, and `E`.

## Interpreter Shell

The interpreter borrows the immutable program root. Interpretation does not
mutate the program.

```rust
pub struct Interpreter<'ir, S, F, C, E, V> {
    pipeline: &'ir Pipeline<S>,
    frames: Vec<F>,
    envs: EnvStack<V>,
    fuel: Option<u64>,
    _marker: PhantomData<(C, E)>,
}
```

Frames store IR ids and resolve them through the interpreter's borrowed
pipeline. They do not store references into the IR.

Root completion is success. If the root frame returns `FrameEffect::Complete(c)`,
the shell returns `Ok(Some(c))` no matter which completion variant it is. If
the frame stack is empty before any completion, `run` returns `Ok(None)`.

The driver applies completion effects in a loop, not through Rust recursion.
The pending effect binding is mutable because parent resume can produce a new
effect to apply immediately; the effect value is not mutated in place.

```rust
fn apply_effect(&mut self, effect: FrameEffect<F, C>) -> Result<StepResult<C>, E> {
    let mut pending = effect;

    loop {
        match pending {
            FrameEffect::Continue(frame) => {
                self.frames.push(frame);
                return Ok(StepResult::Running);
            }
            FrameEffect::Push { parent, child } => {
                self.frames.push(parent);
                self.frames.push(child);
                return Ok(StepResult::Running);
            }
            FrameEffect::Complete(completion) => {
                let Some(parent) = self.frames.pop() else {
                    return Ok(StepResult::Done(completion));
                };
                pending = parent.resume(completion, self)?;
            }
        }
    }
}
```

## Lift and Project Algebra

`Frame`, `Completion`, and `Error` are all composed through lift/project.

For transparent bubbling, failed projection must preserve the original value:

```rust
pub trait ProjectOrSelf<To>: Sized {
    fn project_or_self(self) -> Result<To, Self>;
}

impl<T, To> ProjectOrSelf<To> for T
where
    T: TryProjectTo<To, Error = T>,
{
    fn project_or_self(self) -> Result<To, Self> {
        self.try_project_to()
    }
}
```

The bubbling pattern is:

```rust
match completion.project_or_self::<LocalCompletion>() {
    Ok(local) => handle(local),
    Err(original) => Ok(FrameEffect::Complete(original)),
}
```

This rule applies equally to interpreter-provided completions and
dialect-provided completions. Standard completions are not privileged by the
shell; they are just reusable variants supplied by the interpreter crate.

## Location Reporting

`Frame` does not require all frames to expose a location. Instead, location
reporting is a separate trait.

```rust
pub trait HasLocation {
    fn location(&self) -> Location;
}
```

Standard frames should implement `HasLocation`. Helpers can use it to build
standard protocol errors, but experimental frames are not forced to expose a
location while the API is still evolving.

## Interpreter Error

The interpreter crate supplies standard protocol and IR errors. Error
composition uses `thiserror` plus lift/project.

```rust
#[derive(Debug, thiserror::Error)]
pub enum InterpreterError {
    #[error("empty frame stack")]
    EmptyFrameStack,

    #[error("unexpected completion at {location:?}; expected {expected}")]
    UnexpectedCompletion {
        location: Location,
        expected: &'static str,
    },

    #[error("unhandled completion at root frame")]
    UnhandledCompletion,

    #[error("invalid env index {index}")]
    InvalidEnvIndex { index: usize },

    #[error("unbound SSA value {value:?}")]
    UnboundValue { value: SSAValue },

    #[error("arity mismatch: expected {expected}, got {got}")]
    ArityMismatch { expected: usize, got: usize },

    #[error("fuel exhausted")]
    FuelExhausted,

    #[error("missing compile stage {stage:?}")]
    MissingStage { stage: CompileStage },

    #[error("unknown function {function:?}")]
    UnknownFunction { function: Function },

    #[error("function {function:?} has no staged function at {stage:?}")]
    MissingStagedFunction {
        function: Function,
        stage: CompileStage,
    },

    #[error("unknown staged function {function:?} at {stage:?}")]
    UnknownStagedFunction {
        function: StagedFunction,
        stage: CompileStage,
    },

    #[error("staged function {function:?} has no live specialization")]
    NoLiveSpecialization { function: StagedFunction },

    #[error("staged function {function:?} has {count} live specializations")]
    AmbiguousLiveSpecialization {
        function: StagedFunction,
        count: usize,
    },

    #[error("unknown specialized function {function:?} at {stage:?}")]
    UnknownSpecializedFunction {
        function: SpecializedFunction,
        stage: CompileStage,
    },
}
```

`UnhandledCompletion` should be rare because root completion is final. It is
still useful for helper APIs or tests that require a specific completion.

A composed error can use `thiserror` ergonomics and still participate in the
algebra:

```rust
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error(transparent)]
    Interpreter(#[from] InterpreterError),

    #[error(transparent)]
    Scf(#[from] ScfError),
}
```

The composed crate also implements:

```rust
impl TryLiftFrom<InterpreterError> for MyError { ... }
impl TryProjectTo<InterpreterError> for MyError { type Error = MyError; ... }
```

## Standard Completion

The interpreter crate supplies standard completions symmetric with its standard
frames.

```rust
pub enum StandardCompletion<V> {
    StatementDone,
    BlockDone,
    RegionDone,
    GraphDone,
    FunctionReturned(V),
}
```

These variants are tagged protocol completions, not generic `Unit` or
`Value(V)`. Blocks and regions are traversal containers; they do not
inherently produce values. Values come from semantic statements such as
function return or dialect-specific yield statements.

Dialect completions stay separate:

```rust
pub enum ScfCompletion<V> {
    Yield(V),
}
```

A composed language defines a total completion enum:

```rust
pub enum MyCompletion<V> {
    Standard(StandardCompletion<V>),
    Scf(ScfCompletion<V>),
}
```

Standard frames consume only completion variants that belong to their own
protocol. They bubble every other completion unchanged, including standard
completions that they do not own. For example, `BlockFrame` consumes
`StatementDone`, but bubbles `FunctionReturned(v)` and `ScfCompletion::Yield(v)`.

## Statement Evaluation

Statement evaluation is exposed as a capability on the interpreter. It is the
bridge between IR statement definitions and frame-stack transitions.

The shell driver does not call dialect `Interpretable` impls directly. The
flow is:

1. The shell pops a frame and calls `frame.step(self)`.
2. A `BlockFrame` at `Traversal::Active(statement)` calls
   `interp.dispatch_statement(location, env)`.
3. `dispatch_statement` resolves the statement definition from the immutable
   pipeline and dispatches to the dialect `Interpretable` impl.
4. The dialect impl returns a `StatementEffect`.
5. `BlockFrame` turns that statement-level outcome into a frame-stack
   `FrameEffect`.

This keeps block traversal owned by `BlockFrame`, while statement semantics are
owned by the dialect.

```rust
pub enum StatementEffect<F, C, V> {
    Done,
    Push(F),
    Complete(C),
    Jump { target: Block, args: Vec<V> },
}

pub trait StatementDispatch<F, C, E, V> {
    fn dispatch_statement(
        &mut self,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, V>, E>;
}
```

`StatementEffect` is not a frame-stack effect. It is the result of evaluating one
IR statement at one active statement location. It does not mention the parent
frame because the active `BlockFrame` is still responsible for deciding how the
block traversal advances.

`Done` means the statement is atomic and completed normally. It has already
performed any SSA writes through the env capability.

`Push(child)` means the statement is non-atomic and needs a child frame. The
standard `BlockFrame` pushes a `StatementFrame` with `pending_child = Some(child)`.
The `StatementFrame` then pushes the actual child on its first step.

`Complete(c)` means the statement produced a completion that should leave the
current statement immediately. Examples include `scf.yield` and function
return statements. `BlockFrame` completes immediately with `c`; it does not
insert a `StatementFrame` boundary.

`Jump { target, args }` is consumed directly by `BlockFrame`. CFG jumps are
local block traversal effects, not nonlocal completions.

Examples:

```rust
arith.add      -> StatementEffect::Done
scf.for        -> StatementEffect::Push(ScfForFrame)
scf.yield      -> StatementEffect::Complete(ScfCompletion::Yield(values))
cf.br          -> StatementEffect::Jump { target, args }
function.call  -> StatementEffect::Push(CallFrame)
function.return -> StatementEffect::Complete(StandardCompletion::FunctionReturned(v))
```

### Decision: Direct Complete vs StatementFrame Boundary

The choice is whether `StatementEffect::Complete(c)` should pass through a
`StatementFrame`.

Passing it through a `StatementFrame` gives every statement a uniform frame
lifecycle, but it adds overhead for control statements and makes the
statement-frame boundary look semantic when it is just forwarding.

Completing directly keeps nonlocal control flow direct and matches transparent
bubbling. The recommendation is to complete directly.

## StatementFrame

`StatementFrame` exists only for non-atomic statements that push child frames.
Atomic statements use the `BlockFrame` fast path.

```rust
pub struct StatementFrame<F> {
    pub location: Location,
    pub env: EnvIndex,
    pub pending_child: Option<F>,
}
```

On its first `step`, a `StatementFrame` with a pending child returns:

```rust
FrameEffect::Push {
    parent: statement_frame_without_pending,
    child,
}
```

On `resume`, it consumes `StandardCompletion::StatementDone` and completes with
`StatementDone` for its parent `BlockFrame`. Any other completion bubbles
unchanged.

This keeps the stack shape explicit for non-atomic statements:

```text
BlockFrame(active stmt)
  -> StatementFrame(stmt, pending child)
    -> child frame
```

Simple statements remain fast:

```text
BlockFrame(active stmt) -> dispatch_statement Done -> advance
```

## BlockFrame

`BlockFrame` is a standard CFG traversal frame. It owns block-entry argument
binding and intra-block jumps.

```rust
pub struct BlockFrame<V> {
    pub stage: CompileStage,
    pub block: Block,
    pub traversal: Traversal<Statement>,
    pub env: EnvIndex,
    pub args: Option<Vec<V>>,
}
```

`args` is only meaningful at `Traversal::Entry`. On entry, the frame checks
block argument arity, writes incoming values to block argument SSA values in
`env`, clears `args`, and moves to the first statement or exit.

`BlockFrame` binds block arguments itself instead of requiring parent frames to
bind them. This centralizes the standard SSA block-entry convention and avoids
duplicating binding logic in functions, regions, loops, and CFG jumps.

At `Traversal::Active(statement)`, `BlockFrame` calls `dispatch_statement` directly.

```rust
match interp.dispatch_statement(location, env)? {
    StatementEffect::Done => advance_to_next_statement_or_exit(),
    StatementEffect::Jump { target, args } => enter_target_block(target, args),
    StatementEffect::Push(child) => push_statement_frame_with_child(child),
    StatementEffect::Complete(completion) => FrameEffect::Complete(completion),
}
```

At `Traversal::Exit`, it completes with:

```rust
StandardCompletion::BlockDone
```

When resumed, `BlockFrame` consumes only `StandardCompletion::StatementDone` and
then advances. All other completions bubble unchanged.

### Decision: Explicit Exit Tick

After the last statement completes, `BlockFrame` should move to
`Traversal::Exit` and return `FrameEffect::Continue(self)` rather than immediately
returning `BlockDone`.

The alternative is faster by one driver tick, but the explicit exit state makes
`BlockExit` observable to tracing, breakpoints, and diagnostics. The
recommendation is to keep the explicit exit tick.

## RegionFrame

`RegionFrame` is a standard sequential region traversal frame following CFG
container conventions.

```rust
pub struct RegionFrame<V> {
    pub stage: CompileStage,
    pub region: Region,
    pub traversal: Traversal<Block>,
    pub env: EnvIndex,
    pub entry_args: Option<Vec<V>>,
}
```

At `Traversal::Entry`, it moves to the first block or exits. If `entry_args`
are present, they are passed to the first `BlockFrame`.

At `Traversal::Active(block)`, it pushes a standard `BlockFrame`.

On `resume`, it consumes `StandardCompletion::BlockDone` and advances to the next
block or exit. All other completions bubble unchanged.

At `Traversal::Exit`, it completes with:

```rust
StandardCompletion::RegionDone
```

This frame is intentionally conservative. More sophisticated region semantics,
such as graph regions, dominance-sensitive traversal, or worklist traversal,
should be separate frame types.

## Graph Frames

The interpreter crate may also provide standard `DiGraphFrame` and
`UnGraphFrame` later. They should follow the same shape:

- `Traversal::Entry`
- `Traversal::Active(statement)`
- `Traversal::Exit`
- consume only the graph-owned completion variants
- bubble all other completions

Graph frames are not part of the first implementation milestone. The first
milestone includes `StatementFrame`, `BlockFrame`, `RegionFrame`, and the
standard function/call frames. Graph traversal can be introduced after block
and region execution is working.

## Function and Call Frames

Function traversal uses entry, active, and exit states just like blocks and
regions.

Standard function frames should be provided, but the interpreter shell should
not be specialized to them. A composed language decides how to include them in
its total frame enum and lift/project algebra.

The standard call convention should be split into reusable frames:

```rust
pub enum Callee {
    Function(Function),
    StagedFunction(StagedFunction),
    SpecializedFunction(SpecializedFunction),
}

pub struct CallFrame<V> {
    pub location: Location,
    pub caller_env: EnvIndex,
    pub callee: Callee,
    pub call_stage: CompileStage,
    pub results: Vec<ResultValue>,
    pub args: Vec<V>,
}

pub struct FunctionFrame<V> {
    pub stage: CompileStage,
    pub function: Function,
    pub traversal: Traversal<StagedFunction>,
    pub args: Vec<V>,
}

pub struct StagedFunctionFrame<V> {
    pub stage: CompileStage,
    pub function: StagedFunction,
    pub traversal: Traversal<SpecializedFunction>,
    pub args: Vec<V>,
}

pub struct SpecializedFunctionFrame<V> {
    pub stage: CompileStage,
    pub function: SpecializedFunction,
    pub state: SpecializedFunctionState<V>,
}

pub enum SpecializedFunctionState<V> {
    Entry {
        args: Vec<V>,
    },
    Active {
        traversal: Traversal<Statement>,
        env: EnvIndex,
    },
}
```

`SpecializedFunctionFrame` uses an explicit state enum rather than optional
`env` and `args` fields. In `Entry`, it has call arguments but has not yet
created the callee activation. In `Active`, it always has a live `EnvIndex`
and traverses the specialized function body statement.

This avoids invalid combinations like `env = None` after entry or
`args = Some(_)` while already active.

The active location is:

```rust
Position::SpecializedFunction {
    function,
    traversal: Traversal::Active(body_statement),
}
```

`CallFrame` is the statement-level frame for a standard call statement. It
stores the caller environment and caller result slots. It pushes the function
dispatch frame chain for its `callee`.

`FunctionFrame` and `StagedFunctionFrame` perform standard dispatch. Their
`Active(child)` locations identify the selected child until that child
completes. They carry the call arguments forward until a
`SpecializedFunctionFrame` owns the callee activation.

`SpecializedFunctionFrame` owns standard activation. In `Entry`, it pushes a
new env, binds function arguments, resolves the specialized function body
statement, and transitions to `Active`. In `Active`, it traverses that body
statement. On function return, it pops the callee env and completes upward
with `StandardCompletion::FunctionReturned(v)`.

`CallFrame` consumes `StandardCompletion::FunctionReturned(v)`, writes `v` into
the caller result slots in `caller_env`, and completes with
`StandardCompletion::StatementDone`.

If a specialized function is used as the root frame, then
`FunctionReturned(v)` can be the final root completion.

### Function Lookup

The first standard function implementation uses a small interpreter capability
for the existing IR function hierarchy.

```rust
pub trait FunctionLookup {
    fn staged_function(
        &self,
        function: Function,
        stage: CompileStage,
    ) -> Result<StagedFunction, InterpreterError>;

    fn unique_specialization(
        &self,
        function: StagedFunction,
        stage: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError>;

    fn specialized_body(
        &self,
        function: SpecializedFunction,
        stage: CompileStage,
    ) -> Result<Statement, InterpreterError>;
}
```

`staged_function` uses `Pipeline::function_info(function)` followed by
`FunctionInfo::staged_function(stage)`.

`unique_specialization` uses `StagedFunctionInfo::unique_live_specialization()`.
The first implementation intentionally requires exactly one live
specialization. Signature-based overload resolution can be added later without
changing the frame protocol.

`specialized_body` uses `SpecializedFunctionInfo::body()`.

The `StagedFunctionInfo` and `SpecializedFunctionInfo` lookups are stage-info
lookups, so the implementation should use the existing `kirin-ir`
`StageDispatch`/`StageAction` machinery over `S::Languages` rather than adding
a parallel dispatch mechanism.

These methods return `InterpreterError`. Standard function frames lift that
error into the total error type with the usual error algebra.

Frame behavior:

- `CallFrame::step` pushes `FunctionFrame`, `StagedFunctionFrame`, or
  `SpecializedFunctionFrame` depending on its `Callee`.
- `FunctionFrame::step` resolves `Function -> StagedFunction`, moves to
  `Traversal::Active(staged_function)`, and pushes `StagedFunctionFrame`.
- `StagedFunctionFrame::step` resolves
  `StagedFunction -> SpecializedFunction`, moves to
  `Traversal::Active(specialized_function)`, and pushes
  `SpecializedFunctionFrame`.
- `SpecializedFunctionFrame::step` resolves the body statement, creates the
  callee env, binds arguments, and traverses the body.

### Decision: Where Caller Results Are Written

The choice is whether the callee frame writes directly to caller result slots
or whether a call frame handles the returned value.

Writing directly from the callee frame is shorter but mixes callee activation
with caller statement semantics. A separate `CallFrame` keeps the standard call
statement protocol local to the call statement. The recommendation is to have
`CallFrame` write caller results after receiving `FunctionReturned(v)`.

## Dialect Semantics

Dialect statement evaluation returns `StatementEffect`, not `FrameEffect`. This is
because dialect statement semantics run at the active statement boundary, while
`FrameEffect` is the protocol for the current frame stack.

The dialect trait is implemented for a dialect language type, not for frames.
`self` is the statement definition stored in the IR. The impl can read and
write SSA values through interpreter capabilities, construct child frames, and
construct completions.

A rough dialect trait may look like this:

```rust
pub trait Interpretable<I, F, C, E, V>: Dialect {
    fn interpret(
        &self,
        interp: &mut I,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, V>, E>;
}
```

The interpreter shell provides `StatementDispatch` by resolving an active statement
location into a statement definition and calling that trait:

```rust
impl<'ir, S, F, C, E, V> StatementDispatch<F, C, E, V>
    for Interpreter<'ir, S, F, C, E, V>
{
    fn dispatch_statement(
        &mut self,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, V>, E> {
        let statement = location.active_statement()?;
        let definition = self.definition_at(location.stage, statement)?;
        definition.interpret(self, location, env)
    }
}
```

`StatementDispatch` should be implemented using the existing `kirin-ir` stage
dispatch APIs. The interpreter resolves `location.stage` through
`Pipeline::stage`, then uses `StageDispatch`/`StageAction` over `S::Languages`
to run the action against the concrete `StageInfo<L>` that owns the statement.
The important boundary is that `StatementDispatch` belongs to the interpreter,
and dialect `Interpretable` belongs to statement definitions.

`StatementEffect` then tells `BlockFrame` what happened:

- `Done`: advance to the next statement or block exit.
- `Push(frame)`: push a `StatementFrame` with this pending child frame.
- `Complete(completion)`: complete the current block frame with that
  completion.
- `Jump { target, args }`: enter the target block through the current
  `BlockFrame`.

Closed control protocols should be implemented on a dialect enum that contains
the full cycle. For SCF, semantics should be defined on at least:

```rust
pub enum SCF<T> {
    For(For<T>),
    Yield(Yield<T>),
}
```

and likely:

```rust
pub enum SCF<T> {
    For(For<T>),
    IfElse(If<T>),
    Yield(Yield<T>),
}
```

This matters because the operational protocol for `scf.for` is not closed
without `scf.yield`. The `For` statement pushes an SCF loop frame. The `Yield`
statement completes with an SCF yield completion. The `ForFrame` consumes that
completion and decides whether to iterate, exit, or error.

## SCF Example

SCF defines local frame and completion types:

```rust
pub enum ScfFrame<V> {
    If(ScfIfFrame<V>),
    For(ScfForFrame<V>),
}

pub enum ScfCompletion<V> {
    Yield(V),
}
```

`scf.yield` is a statement, not a generic shell effect. Its interpretation
returns:

```rust
StatementEffect::Complete(ScfCompletion::Yield(values).lift())
```

The active `BlockFrame` receives this completion and bubbles it because it does
not consume SCF completions. The enclosing `ScfForFrame` or `ScfIfFrame`
projects it to `ScfCompletion::Yield` and handles it.

Concrete `scf.for` roughly behaves as:

1. `scf.for` interpretation reads loop operands and returns
   `StatementEffect::Push(ScfForFrame)`.
2. `BlockFrame` pushes a `StatementFrame` with the pending `ScfForFrame`.
3. `StatementFrame` pushes `ScfForFrame`.
4. `ScfForFrame::step` checks the loop condition.
5. If true, it pushes a standard `BlockFrame` for the loop body.
6. If false, it writes final results and completes with `StatementDone`.
7. The body `BlockFrame` bubbles `ScfCompletion::Yield(values)` back to
   `ScfForFrame`.
8. `ScfForFrame::resume` updates carried state and either pushes the body
   again or completes with `StatementDone`.

The shell does not inspect `Yield`. It only transfers completions from child
frames to parent frames.

## Composition Example

A composed language defines total frame, completion, and error enums:

```rust
pub enum MyFrame<V> {
    Statement(StatementFrame<MyFrame<V>>),
    Block(BlockFrame<V>),
    Region(RegionFrame<V>),
    Call(CallFrame<V>),
    Function(FunctionFrame<V>),
    StagedFunction(StagedFunctionFrame<V>),
    SpecializedFunction(SpecializedFunctionFrame<V>),
    Scf(ScfFrame<V>),
}

pub enum MyCompletion<V> {
    Standard(StandardCompletion<V>),
    Scf(ScfCompletion<V>),
}

#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error(transparent)]
    Interpreter(#[from] InterpreterError),

    #[error(transparent)]
    Scf(#[from] ScfError),
}
```

The composed crate implements lift/project for each local type. The final
`Frame` impl for `MyFrame` is a small variant dispatch:

```rust
impl<I, V> Frame<I, MyFrame<V>, MyCompletion<V>, MyError> for MyFrame<V>
where
    BlockFrame<V>: Frame<I, MyFrame<V>, MyCompletion<V>, MyError>,
    ScfFrame<V>: Frame<I, MyFrame<V>, MyCompletion<V>, MyError>,
    // ...
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<MyFrame<V>, MyCompletion<V>>, MyError> {
        match self {
            MyFrame::Block(frame) => frame.step(interp),
            MyFrame::Scf(frame) => frame.step(interp),
            // ...
        }
    }

    fn resume(
        self,
        completion: MyCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<MyFrame<V>, MyCompletion<V>>, MyError> {
        match self {
            MyFrame::Block(frame) => frame.resume(completion, interp),
            MyFrame::Scf(frame) => frame.resume(completion, interp),
            // ...
        }
    }
}
```

This dispatch is mechanical and can be generated later. The important part is
that each frame implementation receives the total completion type directly and
owns its own projection/bubbling behavior.

## Decision Log

### Returned Effects vs Direct Shell Mutation

Frames return stack effects instead of pushing and popping frames directly.
This centralizes stack discipline in the shell while still letting frames read
and write SSA state through interpreter capabilities.

Recommendation: return `FrameEffect<F, C>`.

### Consuming Frames by Value

`step` and `resume` consume `self`. This avoids borrowing the top frame and the
interpreter mutably at the same time. The driver pops the frame, steps it, and
applies the returned effect.

The extra pop/push is expected to be cheap. `Vec::pop` and `Vec::push` are
O(1), and Rust moves are not deep copies.

Recommendation: consume frames by value.

### Immediate Resume vs Inbox

Child completion immediately resumes the parent frame. The shell does not
store a generic inbox.

This keeps parent semantic state inside the parent frame and avoids temporary
states where a frame sits on the stack with pending data outside itself.

Recommendation: immediate `parent.resume(completion, interp)`.

### Traversal Frames Bubble Unknown Completions

Traversal frames consume only variants they own and bubble everything else.
This lets dialect control flow pass through standard block and region frames
without teaching those frames about every dialect completion.

Recommendation: project owned completion, otherwise return `FrameEffect::Complete`.

### Standard Completion Variants

The interpreter crate supplies `StandardCompletion` because it supplies standard
frames. This avoids forcing every composed language to invent its own
equivalent `BlockDone`, `StatementDone`, or `FunctionReturned`.

Recommendation: provide builtin, tagged completions.

### Mandatory Statement Fast Path

`BlockFrame` calls `dispatch_statement` directly. Atomic statements return `Done`
and the block advances without pushing `StatementFrame`.

Non-atomic statements return `Push(child)`, and the block pushes a
`StatementFrame` to preserve statement lifecycle.

Recommendation: mandatory fast path from the start.

### CFG Jumps

CFG jumps are returned as `StatementEffect::Jump`, not as completions. The active
`BlockFrame` owns block traversal and consumes jumps directly.

Recommendation: direct `StatementEffect::Jump`.

### Block Argument Binding

`BlockFrame` binds block arguments at entry instead of requiring the parent to
bind them before pushing the frame.

Recommendation: centralize block-entry binding in `BlockFrame`.

### Root Completion

Any completion from the root frame is final. The shell does not require a
terminal marker or inspect the completion.

Recommendation: root `Complete(c)` returns `Ok(Some(c))`.

### Naming

The first implementation uses the settled names from this document:

- `Frame`
- `FrameEffect`
- `StatementDispatch::dispatch_statement`
- `StatementEffect`
- `Completion`
- `StandardCompletion`
- `InterpreterError`
- `ProjectOrSelf`
- `HasLocation`
- `Env`
- `EnvIndex`
- `Location`
- `Position`
- `Traversal`
- `Callee`
- `Interpretable::interpret`

## Deferred Work

- Add signature-based specialization dispatch after the simple
  `unique_live_specialization()` path is implemented.
- Add `DiGraphFrame` and `UnGraphFrame` after block and region execution are
  working.
- Add derive or helper macros for total frame, completion, and error
  composition after the manual pattern stabilizes.
