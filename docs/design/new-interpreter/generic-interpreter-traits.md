# Generic Interpreter Traits

This file defines the shared vocabulary used by concrete and abstract
interpreters. The traits are intentionally generic over the total frame,
completion, error, and transfer types.

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
    Statement {
        statement: Statement,
    },
    DiGraph {
        graph: DiGraph,
        traversal: Traversal<GraphNode>,
    },
    UnGraph {
        graph: UnGraph,
        traversal: Traversal<GraphNode>,
    },
}
```

`Traversal::Active(child)` always means the currently active child. For a block,
that child is the current statement. For a region, that child is the current
block. For function dispatch, `Function`, `StagedFunction`, and
`SpecializedFunction` are distinct positions because they represent distinct
dispatch contexts.

Semantic phases like `ForCondition`, `WaitingForYield`, or `DispatchingCall`
do not belong in `Location`. They belong in frame state.

## Env

SSA activation storage is owned by the interpreter shell. Frames that need an
activation store carry an `EnvIndex`.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EnvIndex(usize);

pub trait Env<V> {
    type Error;

    fn alloc(&mut self) -> EnvIndex;
    fn free(&mut self, env: EnvIndex) -> Result<(), Self::Error>;

    fn read(&self, env: EnvIndex, value: SSAValue) -> Result<V, Self::Error>;
    fn write(
        &mut self,
        env: EnvIndex,
        value: SSAValue,
        data: V,
    ) -> Result<(), Self::Error>;
}

pub trait EnvStack<V>: Env<V> {
    fn push(&mut self) -> EnvIndex;
    fn pop(&mut self) -> Result<(), Self::Error>;
    fn current(&self) -> Result<EnvIndex, Self::Error>;
}
```

`EnvIndex` is an opaque slot index. A plain index is enough for now; it does
not need a generation counter. The standard env implementation validates that
an index is live.

The base `Env` trait is not stack-shaped. This matters for abstract
interpreters, where the driver stores frame-node tables rather than a LIFO call
stack. Concrete interpreters can additionally implement `EnvStack`; in that
case `pop` only removes the top environment.

`read` and `write` take an explicit `EnvIndex`, and writes are allowed to any
live environment index. This lets a parent frame resume and continue writing to
its own activation after a child call has pushed and popped another activation.

## FrameEffect

`FrameEffect` is only the frame-structure protocol. It does not contain env
operations, SSA writes, or pipeline access commands.

```rust
pub enum FrameEffect<F, C> {
    Continue(F),
    Push { parent: F, child: F },
    Complete(C),
}
```

Env mutation happens through `&mut I` capability traits on the interpreter.
This keeps the returned effect small and avoids turning `FrameEffect` into a
command language.

`Push` pushes exactly one child. If a frame needs to enter several layers, it
does so over multiple driver ticks.

```text
Continue(f)             => push f
Push { parent, child }  => push parent, then child
Complete(c), parent     => resume parent with c
Complete(c), root       => final completion
```

`Continue(f)` means the same frame remains active and runs on the next driver
tick. Both `step` and `resume` may return any `FrameEffect`, including `Push`.
This is needed for frames such as `scf.for`, where receiving a body yield may
immediately push the next loop-body block frame.

The effect type is driver-neutral. A concrete driver applies it to a `Vec<F>`.
An abstract driver applies it to frame tables and dependency indexes.

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

`F` is the total frame type, `C` is the total completion type, and `E` is the
total error type. Standard frames and dialect frames are both written against
arbitrary total `F`, `C`, and `E`.

## Lift And Project

`Frame`, `Completion`, `Error`, and summaries are composed through lift/project.
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
dialect-provided completions.

## Location Reporting

`Frame` does not require all frames to expose a location. Instead, location
reporting is a separate trait.

```rust
pub trait HasLocation {
    fn location(&self) -> Location;
}
```

Standard frames should implement `HasLocation`. Helpers can use it to build
traces, diagnostics, breakpoints, and abstract node keys. Dialect frames should
implement it when they have a meaningful current traversal location.

## InterpreterError

The interpreter crate provides a reusable core error type. It also participates
in lift/project composition.

```rust
#[derive(thiserror::Error, Debug)]
pub enum InterpreterError {
    #[error("expected active statement at {location:?}")]
    ExpectedActiveStatement { location: Location },

    #[error("expected active block at {location:?}")]
    ExpectedActiveBlock { location: Location },

    #[error("invalid env index {index:?}")]
    InvalidEnvIndex { index: EnvIndex },

    #[error("unexpected completion at {location:?}")]
    UnexpectedCompletion { location: Location },

    #[error("empty frame stack")]
    EmptyFrameStack,
}
```

Dialect errors are lifted into the language-level total error enum.

## StandardCompletion

The interpreter crate provides standard completion variants symmetric to the
standard frame types.

```rust
pub enum StandardCompletion<V> {
    StatementDone,
    BlockDone,
    RegionDone,
    GraphDone,
    FunctionReturned(V),
}
```

These are not privileged by the shell. They are reusable completion variants
that compose with dialect completions through lift/project.

## StatementEffect

`BlockFrame` owns block traversal, but statement semantics belong to dialects.
The bridge is `StatementEffect`:

```rust
pub enum StatementEffect<F, C, T> {
    Done,
    Push(F),
    Complete(C),
    Transfer(T),
}

pub trait StatementDispatch<F, C, E, T> {
    fn dispatch_statement(
        &mut self,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, T>, E>;
}
```

The variants mean:

- `Done`: atomic statement finished and block traversal should advance.
- `Push(frame)`: non-atomic statement produced a child frame.
- `Complete(completion)`: statement completed the active traversal frame.
- `Transfer(transfer)`: statement-local control transfer handled by the active
  traversal frame.

The transfer payload is specialized to the interpreter/frame family:

```rust
pub enum ConcreteTransfer<V> {
    Jump { target: Block, args: Vec<V> },
}

pub struct BlockTransfer<V> {
    pub target: Block,
    pub args: Vec<V>,
}

pub enum ForwardTransfer<V> {
    Branch(Vec<BlockTransfer<V>>),
}

pub enum BackwardTransfer<D> {
    Branch(Vec<D>),
}
```

Concrete can specialize the transfer type to a single `Jump`. Forward abstract
interpretation can specialize it to a single `Branch`, using a one-edge branch
for unconditional jumps. Backward analyses can use a transfer payload whose
dependencies point backward.

## Interpretable

Dialect statement semantics are exposed through `Interpretable::interpret`.

```rust
pub trait Interpretable<I, F, C, E, T>: Dialect {
    fn interpret(
        &self,
        interp: &mut I,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, T>, E>;
}
```

The interpreter shell provides `StatementDispatch` by resolving an active
statement location into a statement definition and calling that trait:

```rust
impl<'ir, S, F, C, E, V, T> StatementDispatch<F, C, E, T>
    for Interpreter<'ir, S, F, C, E, V>
{
    fn dispatch_statement(
        &mut self,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, T>, E> {
        let statement = location.active_statement()?;
        let definition = self.definition_at(location.stage, statement)?;
        definition.interpret(self, location, env)
    }
}
```

Dialect authors implement `Interpretable` for their statement enums or
statement definitions. Interpreter authors implement the shell and the standard
frames. Dialect authors normally see `StatementDispatch` as a capability on the
interpreter, not as a trait they implement.
