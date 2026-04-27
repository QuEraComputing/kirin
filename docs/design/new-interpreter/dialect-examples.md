# Dialect Examples

This file shows how dialect authors interact with the new interpreter design.
The important split is:

- frames define traversal state and resume logic,
- statements define local semantics through `Interpretable::interpret`,
- language authors compose total frame, completion, error, transfer, and summary
  enums.

## Dialect Semantics

Dialect statement semantics use `Interpretable::interpret`:

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

The variants map to driver behavior:

- `Done`: current statement finished; the active traversal frame advances.
- `Push(frame)`: current statement created a child frame.
- `Complete(completion)`: current statement completed the active traversal
  frame.
- `Transfer(transfer)`: current statement produced traversal-local control
  transfer.

Dialect authors can specialize `Interpretable` on concrete interpreter types,
forward abstract interpreter types, or backward analysis interpreter types. That
specialization controls the total frame type, completion type, error type, and
transfer type.

Closed control protocols should be implemented on a dialect enum that contains
the full cycle. For SCF, semantics should be defined on at least:

```rust
pub enum SCF {
    For(For),
    Yield(Yield),
}
```

or, for convenience:

```rust
pub enum SCF {
    For(For),
    IfElse(IfElse),
    Yield(Yield),
}
```

The reason is that `scf.for` and `scf.yield` jointly define the complete
mutation cycle of the SCF frame. Defining `scf.for` alone is not enough.

## SCF Example

SCF can provide its own frame, completion, and error variants:

```rust
pub enum ScfFrame<V> {
    For(ForFrame<V>),
    If(IfFrame<V>),
}

pub enum ScfCompletion<V> {
    Yield(Vec<V>),
}

pub enum ScfError {
    YieldOutsideScf,
    ArityMismatch,
}
```

The SCF dialect implementation returns statement effects:

```rust
impl<I, F, C, E, V, T> Interpretable<I, F, C, E, T> for SCF
where
    ScfFrame<V>: LiftTo<F>,
    ScfCompletion<V>: LiftTo<C>,
    ScfError: LiftTo<E>,
{
    fn interpret(
        &self,
        interp: &mut I,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, T>, E> {
        match self {
            SCF::For(op) => {
                let frame = ForFrame::new(op, location, env);
                Ok(StatementEffect::Push(frame.lift()))
            }
            SCF::Yield(op) => {
                let values = op
                    .values
                    .iter()
                    .map(|value| interp.read(env, *value))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(StatementEffect::Complete(
                    ScfCompletion::Yield(values).lift(),
                ))
            }
            SCF::IfElse(op) => {
                let frame = IfFrame::new(op, location, env);
                Ok(StatementEffect::Push(frame.lift()))
            }
        }
    }
}
```

`ForFrame::resume` owns the loop-specific interpretation of
`ScfCompletion::Yield`. If the completion is not an SCF yield, it bubbles the
original completion:

```rust
impl<I, F, C, E, V> Frame<I, F, C, E> for ForFrame<V>
where
    C: ProjectOrSelf<ScfCompletion<V>>,
    ForFrame<V>: LiftTo<F>,
{
    fn resume(
        self,
        completion: C,
        interp: &mut I,
    ) -> Result<FrameEffect<F, C>, E> {
        match completion.project_or_self::<ScfCompletion<V>>() {
            Ok(ScfCompletion::Yield(values)) => {
                self.advance_after_body_yield(values, interp)
            }
            Err(original) => Ok(FrameEffect::Complete(original)),
        }
    }
}
```

This keeps the shell generic. The shell does not know that `scf.yield` resumes
an `scf.for`; the SCF frame author defines that protocol.

## Composition Example

A language author composes frames, completions, errors, transfers, and summaries
manually at first. Macros can reduce this boilerplate later.

```rust
pub enum MyFrame<V> {
    Statement(StatementFrame),
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

#[derive(thiserror::Error, Debug)]
pub enum MyError {
    #[error(transparent)]
    Interpreter(#[from] InterpreterError),

    #[error(transparent)]
    Scf(#[from] ScfError),
}

pub enum MyTransfer<V> {
    Concrete(ConcreteTransfer<V>),
}

pub enum MySummary<V> {
    Block(BlockSummary<V>),
    Function(FunctionSummary<V>),
    Scf(ScfSummary<V>),
}
```

Manual `Frame` dispatch for the total frame type is mechanical:

```rust
impl<I, V, C, E> Frame<I, MyFrame<V>, C, E> for MyFrame<V>
where
    StatementFrame: Frame<I, MyFrame<V>, C, E>,
    BlockFrame<V>: Frame<I, MyFrame<V>, C, E>,
    RegionFrame<V>: Frame<I, MyFrame<V>, C, E>,
    CallFrame<V>: Frame<I, MyFrame<V>, C, E>,
    FunctionFrame<V>: Frame<I, MyFrame<V>, C, E>,
    ScfFrame<V>: Frame<I, MyFrame<V>, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<MyFrame<V>, C>, E> {
        match self {
            MyFrame::Statement(frame) => frame.step(interp),
            MyFrame::Block(frame) => frame.step(interp),
            MyFrame::Region(frame) => frame.step(interp),
            MyFrame::Call(frame) => frame.step(interp),
            MyFrame::Function(frame) => frame.step(interp),
            MyFrame::Scf(frame) => frame.step(interp),
            MyFrame::StagedFunction(frame) => frame.step(interp),
            MyFrame::SpecializedFunction(frame) => frame.step(interp),
        }
    }

    fn resume(
        self,
        completion: C,
        interp: &mut I,
    ) -> Result<FrameEffect<MyFrame<V>, C>, E> {
        match self {
            MyFrame::Statement(frame) => frame.resume(completion, interp),
            MyFrame::Block(frame) => frame.resume(completion, interp),
            MyFrame::Region(frame) => frame.resume(completion, interp),
            MyFrame::Call(frame) => frame.resume(completion, interp),
            MyFrame::Function(frame) => frame.resume(completion, interp),
            MyFrame::Scf(frame) => frame.resume(completion, interp),
            MyFrame::StagedFunction(frame) => frame.resume(completion, interp),
            MyFrame::SpecializedFunction(frame) => {
                frame.resume(completion, interp)
            }
        }
    }
}
```

This dispatch is mechanical and can be generated later. The important part is
that each frame implementation receives the total completion type directly and
owns its own projection/bubbling behavior.

## Specializing For Abstract Interpretation

The same dialect can provide different `Interpretable` impls for different
interpreter families. Concrete execution may use `ConcreteTransfer<V>`, while a
forward abstract interpreter may use `ForwardTransfer<AbstractValue>`, and a
backward analysis may use `BackwardTransfer<LivenessRequirement>`.

This means dialect authors can choose different frame types for different
traversal orders. A backward liveness frame does not have to look like a
concrete forward block frame. The common contract is that both are frames,
both return `FrameEffect`, and both compose through the same total-frame and
completion algebra.
