# Concrete Interpreter Design

The concrete interpreter is a deterministic stack machine. It uses the generic
`Frame` protocol, but its driver state is a concrete `Vec<F>` plus an env stack.

## Interpreter Shell

The concrete interpreter borrows the immutable program root. Interpretation
does not mutate the program.

```rust
pub struct Interpreter<'ir, S, F, C, E, V> {
    pub pipeline: &'ir Pipeline<S>,
    pub frames: Vec<F>,
    pub env: EnvStackStore<V>,
    pub error: PhantomData<E>,
    pub completion: PhantomData<C>,
}
```

The concrete shell owns pipeline access, stage access, env access, and the frame
stack. A frame can read and write SSA state through `&mut I` capability traits,
but it cannot mutate the stack directly.

The concrete driver loop repeatedly steps the top frame and applies the returned
effect:

```rust
fn step(&mut self) -> Result<StepResult<C>, E>
where
    F: Frame<Self, F, C, E>,
{
    let frame = self.frames.pop().ok_or(InterpreterError::EmptyFrameStack)?;
    let effect = frame.step(self)?;
    self.apply_effect(effect)
}

fn apply_effect(&mut self, effect: FrameEffect<F, C>) -> Result<StepResult<C>, E>
where
    F: Frame<Self, F, C, E>,
{
    match effect {
        FrameEffect::Continue(frame) => {
            self.frames.push(frame);
            Ok(StepResult::Running)
        }
        FrameEffect::Push { parent, child } => {
            self.frames.push(parent);
            self.frames.push(child);
            Ok(StepResult::Running)
        }
        FrameEffect::Complete(completion) => {
            match self.frames.pop() {
                Some(parent) => {
                    let effect = parent.resume(completion, self)?;
                    self.apply_effect(effect)
                }
                None => Ok(StepResult::Complete(completion)),
            }
        }
    }
}
```

Root completion is success. If the root frame returns
`FrameEffect::Complete(c)`, interpretation returns `c`.

## StatementFrame

`StatementFrame` is a standard adapter frame. It preserves a statement boundary
when a statement returns `StatementEffect::Push(child)`.

```rust
pub struct StatementFrame<F> {
    pub location: Location,
    pub env: EnvIndex,
    pub child: F,
}
```

Without `StatementFrame`, `BlockFrame` would have to know how every child frame
completion should map back into statement completion. `StatementFrame` localizes
that adapter logic.

The fast path is mandatory:

- atomic statements return `StatementEffect::Done` and do not allocate a
  `StatementFrame`.
- non-atomic statements return `StatementEffect::Push(child)`, and `BlockFrame`
  pushes a `StatementFrame` around the child.

This keeps common statement execution cheap while still giving complex
statements an observable frame boundary for tracing, breakpoints, and
diagnostics.

## BlockFrame

`BlockFrame` is the standard linear block traversal frame. It owns the current
statement cursor and the env activation for that block.

```rust
pub struct BlockFrame<V> {
    pub block: Block,
    pub traversal: Traversal<Statement>,
    pub env: EnvIndex,
    pub incoming_args: Vec<V>,
}
```

At `Traversal::Entry`, `BlockFrame` binds block arguments into `env` and moves
to the first statement or `Traversal::Exit`.

At `Traversal::Active(statement)`, `BlockFrame` calls `StatementDispatch`
directly:

```rust
match interp.dispatch_statement(location, env)? {
    StatementEffect::Done => advance_to_next_statement_or_exit(),
    StatementEffect::Transfer(ConcreteTransfer::Jump { target, args }) => {
        enter_target_block(target, args)
    }
    StatementEffect::Push(child) => push_statement_frame_with_child(child),
    StatementEffect::Complete(completion) => FrameEffect::Complete(completion),
}
```

At `Traversal::Exit`, `BlockFrame` returns
`StandardCompletion::BlockDone.lift()`.

The explicit exit tick is recommended. The alternative is faster by one driver
tick, but explicit exit makes `BlockExit` observable to tracing, breakpoints,
and diagnostics.

Concrete `BlockFrame` should be implemented only for a concrete transfer type
that it owns, such as `ConcreteTransfer<V>`. A forward abstract block frame
should use a different transfer type, such as `ForwardTransfer<V>`, and a
backward analysis frame should use a backward transfer type.

## RegionFrame

`RegionFrame` is a standard sequential region traversal frame following CFG
convention. It tracks the current block and delegates block execution to
`BlockFrame`.

```rust
pub struct RegionFrame<V> {
    pub region: Region,
    pub traversal: Traversal<Block>,
    pub env: EnvIndex,
    pub incoming_args: Vec<V>,
}
```

The region frame should not duplicate block traversal logic. It enters a block
by pushing a `BlockFrame`, and it consumes only region-owned completions. Unknown
completions are bubbled with `FrameEffect::Complete(original)`.

## Graph Frames

The interpreter crate may also provide standard `DiGraphFrame` and
`UnGraphFrame` later. They should follow the same shape:

- own graph traversal state,
- push child frames for node execution,
- consume only graph-owned completion variants,
- bubble unknown completions.

Graph frames are not part of the first implementation milestone. The first
standard frames should be `StatementFrame`, `BlockFrame`, `RegionFrame`, and
standard function/call frames. Graph traversal can be introduced after block
and region execution are stable.

## Function And Call Frames

Function and call frames model a standard call convention, but the interpreter
does not require dialects to use this convention.

```rust
pub enum Callee {
    Function(Function),
    StagedFunction(StagedFunction),
    SpecializedFunction(SpecializedFunction),
}

pub struct CallFrame<V> {
    pub call_site: Statement,
    pub callee: Callee,
    pub args: Vec<V>,
    pub caller_env: EnvIndex,
    pub results: Vec<SSAValue>,
}
```

`CallFrame` is the continuation for a call statement. It resolves the callee,
pushes the appropriate function frame, and on return writes results into the
caller env.

```rust
pub struct FunctionFrame<V> {
    pub function: Function,
    pub args: Vec<V>,
}

pub struct StagedFunctionFrame<V> {
    pub stage: CompileStage,
    pub function: StagedFunction,
    pub args: Vec<V>,
}

pub struct SpecializedFunctionFrame<V> {
    pub stage: CompileStage,
    pub function: SpecializedFunction,
    pub state: SpecializedFunctionState<V>,
}

pub enum SpecializedFunctionState<V> {
    Entry { args: Vec<V> },
    Active {
        traversal: Traversal<Statement>,
        env: EnvIndex,
    },
}
```

`SpecializedFunctionFrame` does not use `Option` for env or args. At entry, it
has args and no activation yet. During active execution, it has an env and the
function body traversal state. This makes the state machine explicit.

Function lookup follows the location hierarchy:

```text
Function
  -> StagedFunction
  -> SpecializedFunction
  -> function body frame
```

Executing the specialized function statement pushes the frame implied by the
body semantics:

- block body -> `BlockFrame`
- region body -> `RegionFrame`
- graph body -> graph frame
- dialect-defined body -> dialect-defined frame

Caller results are written by `CallFrame`, not by the callee function frame.
The function frame returns `StandardCompletion::FunctionReturned(value)`. The
call frame projects that completion and writes the value into `caller_env` at
the call results.

This convention keeps callee execution independent of caller result placement.
