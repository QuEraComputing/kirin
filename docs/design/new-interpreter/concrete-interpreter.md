# Concrete Interpreter Design

The concrete interpreter is a deterministic stack machine. It uses the generic
`Frame` protocol, but its driver state is a concrete `Vec<F>` plus an env
store. The common concrete store is stack-shaped because calls push function
activations.

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

Env allocation is not tied to block traversal. Blocks and regions usually carry
the current function activation `EnvIndex`. New envs are created by call/function
boundaries, or by dialect statements that explicitly introduce a new scope or
activation-like object.

The common concrete env store is stack-shaped, but that is a concrete helper,
not part of the shared `Env` trait:

```rust
pub struct EnvStackStore<V> {
    // stack of live SSA stores
}
```

`EnvStackStore` implements `Env<V>` and exposes concrete stack operations such
as `push`, `pop`, and `current`. `pop` only removes the top activation.

Standard block traversal uses a block transfer payload:

```rust
pub enum BlockTransfer<V> {
    Jump {
        target: Block,
        args: Vec<V>,
    },
    Branch {
        true_target: Block,
        true_args: Vec<V>,
        false_target: Block,
        false_args: Vec<V>,
    },
}
```

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
        FrameEffect::Done => {
            match self.frames.pop() {
                Some(parent) => {
                    let effect = parent.resume_done(self)?;
                    self.apply_effect(effect)
                }
                None => Err(InterpreterError::EmptyFrameStack.into()),
            }
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
for tools that want an observable statement continuation.

```rust
pub struct StatementFrame {
    pub location: Location,
    pub env: EnvIndex,
}
```

Normal child-frame completion uses `FrameEffect::Done`, not a completion value.
This is what lets a call frame complete a call statement without inventing a
separate standard completion variant for atomic statement completion.

The fast path is mandatory:

- atomic statements return `StatementEffect::Done` and do not allocate a
  `StatementFrame`.
- non-atomic statements return `StatementEffect::Push(child)`, and the child
  returns `FrameEffect::Done` when the parent statement may advance.

This keeps common statement execution cheap while still giving complex
statements an observable frame boundary for tracing, breakpoints, and
diagnostics.

## BlockFrame

`BlockFrame` is the standard linear block traversal frame. It owns the current
statement cursor and carries the current activation env. It does not allocate or
free that env.

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

Block arguments are written into the existing activation env. In normal
function bodies, all blocks in the same function share the same env index; a CFG
jump updates the target block arguments in that env and continues traversal.

At `Traversal::Active(statement)`, `BlockFrame` calls `StatementDispatch`
directly:

```rust
match interp.dispatch_statement(location, env)? {
    StatementEffect::Done => advance_to_next_statement_or_exit(),
    StatementEffect::Transfer(BlockTransfer::Jump { target, args }) => {
        enter_target_block(target, args)
    }
    StatementEffect::Transfer(BlockTransfer::Branch { .. }) => {
        dispatch_branch_transfer()
    }
    StatementEffect::Push(child) => push_child_frame(child),
    StatementEffect::Complete(completion) => FrameEffect::Complete(completion),
}
```

At `Traversal::Exit`, `BlockFrame` returns
`StandardCompletion::BlockDone.lift()`.

The explicit exit tick is recommended. The alternative is faster by one driver
tick, but explicit exit makes `BlockExit` observable to tracing, breakpoints,
and diagnostics.

Standard `BlockFrame` is implemented for the block transfer type that it owns,
`BlockTransfer<V>`. More specialized forward abstract and backward analysis
frames can still use their own transfer payloads.

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

The region frame should not duplicate block traversal logic or allocate a new
env by default. It carries the active env into each `BlockFrame`, and it
consumes only region-owned completions. Unknown completions are bubbled with
`FrameEffect::Complete(original)`.

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
        env: EnvIndex,
    },
}
```

`SpecializedFunctionFrame` does not use `Option` for env or args. At entry, it
has args and no activation yet. During active execution, it has an env and is
waiting for the body frame to complete. This makes the activation lifecycle
explicit without baking any body traversal strategy into the function frame.

`SpecializedFunctionFrame` is the standard place where a function-call
activation env is created. It allocates an env at entry, binds function
parameters into that env, builds the frame implied by the function body
semantics, and returns `FrameEffect::Push { parent, child }`, where `parent` is
the active function frame and `child` is the body frame. On body completion, it
frees or pops that activation according to the env store policy.

Function lookup follows the location hierarchy:

```text
Function
  -> StagedFunction
  -> SpecializedFunction
  -> function body frame
```

Executing the specialized function statement pushes the frame implied by the
body semantics, passing the function activation env into that frame:

- block body -> `BlockFrame`
- region body -> `RegionFrame`
- graph body -> graph frame
- dialect-defined body -> dialect-defined frame

Caller results are written by `CallFrame`, not by the callee function frame.
The function frame returns `StandardCompletion::FunctionReturned(value)`. The
call frame projects that completion, writes the value into `caller_env` at the
call results, and returns `FrameEffect::Done` so the parent statement traversal
can advance.

This convention keeps callee execution independent of caller result placement.
