# Interpreter

## ConcreteInterpreter

We will learn how the interpreter infrastructure is designed through the example
of the `ConcreteInterpreter`.

`ConcreteInterpreter` is the interpreter that represents the concrete execution
of a program. It traverses a program sequentially from start to end, producing
at each SSA-value the result of executing its corresponding operation.

A `ConcreteInterpreter` comprises the following (wrapping
`ConcreteInterpreterCore`):

```rust
pub struct ConcreteInterpreter<'ir, S: StageMeta, V, E> {
    pipeline: &'ir Pipeline<S>,
    linker: SameStageLinker,
    env: ConcreteEnv<V>,
    frames: Vec<FrameStackItem<V, E>>,
    location: Option<InterpLocation>,
}
```

These fields will be explained as we explore how `ConcreteInterpreter` works.

An implementation for an interpreter will typically have the following three
components:

- Core definition — implemented through [`Interp`](#interp-trait)
- Storage — implemented through [`Env` and `EnvStore`](#env-and-envstore)
- Traversal — implemented through [`Frame`](#frame-trait)

### `Interp` trait

The `Interp` trait is the core of the interpreter infrastructure.
`ConcreteInterpreter` implements it as follows:

```rust
impl<'ir, S, V, E> Interp for ConcreteInterpreter<'ir, S, V, E>
where
    S: StageMeta,
    V: Clone,
    E: From<InterpreterError>,
{
    type Value = V;
    type Error = E;
    type Effect = SparseForwardEffect<V, FrameStackItem<V, E>>;
    type Semantics = ForwardEval;

    /// The stage the current statement belongs to.
    fn stage(&self) -> CompileStage {
        self.location.expect("interp location not set").stage
    }

    /// The statement currently being interpreted.
    fn statement(&self) -> Statement {
        self.location.expect("interp location not set").statement
    }

    /// The current SSA activation.
    fn index(&self) -> EnvIndex {
        self.location.expect("interp location not set").index
    }
}
```

It defines the following types:

- `Value`  — The type that facts can be. For `ConcreteInterpreter` this is the
  `V` type parameter that each SSA-values hold. We can imagine this being `i64`
  for an integer calculator language, or a richer `Value` type that can hold
  integers, floats, bools, lambdas, etc.
- `Error` — The type that errors can be. For `ConcreteInterpreter` this is bound
  to the `E` type parameter. This is usually an enum holding all the different
  error variants that the interpreter may produce. For example, we may have
  errors corresponding to a particular dialect like a division-by-zero error for
  an arithmetic dialect, and we may have errors associated with the
  interpreter’s execution like a name-not-found error.
- `Effect` — What the interpreter produces at each step (e.g. at each program
  point or SSA-value) given the operation at that point. For example, a control
  flow operation may produce a `Jump` or `Branch` effect. Then, depending on the
  engine used by the interpreter, the engine will take care of performing the
  appropriate action based on the effect produced. For `ConcreteInterpreter`
  the effects are `SparseForwardEffect`.
- `Semantics` — `Semantics` must implement `SemanticKey` which contains no data
  nor methods. It is simply the name of the kind of interpretation or analysis
  done. For example, a `SparseForwardSemantic` means traversal is done per
  SSA-value, from start to end, whereas `DenseBackwardSemantic` mean traversal
  is done per program point, from end to start. For `ConcreteInterpreter` the
  `Semantics` is `ForwardEval` which implements `SparseForwardSemantic`.

In addition, `Interp` provides three methods relating to identifying the current
statement being interpreted (see the [above code block](#interp-trait)).

### `Env` and `EnvStore`

#### `Env` trait

An interpreter implementing `Interp` can be extended with various capabilities,
also defined as traits. One of which is the `Env` trait, which provides the
capability for **using** an environment (i.e. does not implement the
environment’s store). This means reading and writing facts at the `Anchor`
points found in the program. These anchor points can be, for example SSA-values
(sparse), or program points (dense). This just described is defined as follows:

```rust
pub trait Env: Interp {
    type Anchor: LatticeAnchor;

    /// Read the fact anchored at `anchor` in an activation.
    fn env_read(&self, index: EnvIndex, anchor: Self::Anchor) -> Result<Self::Value, Self::Error>;
    /// Write the fact anchored at `anchor` in an activation.
    fn env_write(
        &mut self,
        index: EnvIndex,
        anchor: Self::Anchor,
        data: Self::Value,
    ) -> Result<(), Self::Error>;
}
```

#### `EnvStore` struct

To provide realizations of these methods, an interpreter needs an underlying
`EnvStore` that actually holds the facts. We saw in [the
definition](#concreteinterpreter) of the `ConcreteInterpreter` that it holds a
`ConcreteEnv<V>` which is defined as `EnvStore<Infallible, SSAValue, V>`.

```rust
pub struct EnvStore<K, A, V>
where
    A: Eq + Hash,
{
    context_indices: HashMap<K, EnvIndex>,
    environments: Vec<Option<Environment<K, A, V>>>,
```

An `EnvStore` contains a directory of environments, and the environments
themselves. This directory maps some kind of key to an environment in the store.
They keys (bound by the `K` type variable) are used to alias different entities
to the same environment. For example, a given analysis may want to always access
the same environment for a function, regardless of where it is being called from
or what arguments are being passed. The key in this case would be some
representation of a function such that whenever we retrieve a fact for that
function we always access the same environment. For `ConcreteInterpreter` we do
not want any such aliasing, which is why `K` is set to `Infallible`, i.e. a
value that can never be created. This does not mean we cannot access any
environments in the `EnvStore`. We can still access the environments with their
corresponding `EnvIndex` which indexes into the `environments` field.

```rust
struct Environment<K, A, V>
where
    A: Eq + Hash,
{
    key: Option<K>,
    facts: FactStore<A, V>,
}
```

An `Environment` is essentially a map from the anchor points to the facts. For
the `ConcreteInterpreter` this would be a map from SSA-values to the values they
get evaluated to. An `Environment` also holds the `key` that fetches it from its
`EnvStore` to be able to free it in constant time.

Note that an `EnvStore` has multiple environments. This is meant to capture the
fact that a SSA-value can hold different facts at the same moment. For example,
a function argument can hold different values each time the function is called.
When the function is recursively called, then this argument has multiple
instantiations live at the same time. We therefore need separate environments
for each instance. If our analysis does not need to distinguish between the
calls, that’s where the Key comes in to alias the environments.

### `Frame` trait

A `Frame` is a resumable continuation. In other words, it’s an object that
represents what piece of a program are we executing/analyzing, and where do we
need to continue once we’re done there. This allows us to represent nested
(possibly recursive) structures of our program as a memory-allocated stack
rather than through the runtime call stack. For our `ConcreteInterpreter`, this
is defined [above](#concreteinterpreter) in the `frames` field defined as
`Vec<FrameStackItem<V, E>>`.

A `Frame` is defined with the following methods:

```rust
pub trait Frame<I: FrameEngine, F = Self>: Sized {
    /// The completion payload this frame family bubbles to parents/root.
    type Completion;

    /// Do this frame's next unit of work.
    fn step_into(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion, F>, I::Error>;

    /// Our child frame finished without a payload.
    /// Let's resume where we left off.
    fn resume_done_into(
        self,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion, F>, I::Error>;

    /// Our child frame finished and produced a payload.
    /// Let's resume where we left off.
    fn resume_into(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion, F>, I::Error>;
}
```

The methods of `Frame` all return a `FrameEffect` which tells the interpreter
what it should do next with the frame stack.

```rust
pub enum FrameEffect<P, C, F = P> {
    /// Keep running the current frame.
    Continue(P),
    /// Suspend `parent` and run `child` first.
    Push { parent: P, child: F },
    /// This frame finished with no payload; its parent's
    /// [`Frame::resume_done_into`] is called.
    Done,
    /// This frame produced a completion `C`; its parent's
    /// [`Frame::resume_into`] is called
    Complete(C),
}
```

The `drive_frames` function implements the frame driver loop that takes a frame
stack and executes it, running `Frame`'s methods based on the previous
iteration’s `FrameEffect`.

However, this just executes the mechanical process of manipulating a frame
stack. It does not describe the semantics behind pushing or completing a frame:
which piece of the program a frame is walking, what it must do when its child
finishes, and what its own completion means. In other words, when a function
call causes a frame to be created, a `FrameEffect::Push` will be created, but
the information held by that function call’s frame is captured elsewhere. That
information lives in a `CallFrame`, and its implementation of `Frame`’s methods
is what translates the “semantic” `CallFrame` into the “driver” `FrameEffect`.

For our `ConcreteInterpreter`, we define many such semantic frames to describe
how the various elements of the IR should be traversed. These include the
representation walkers `BlockFrame`, `CFGFrame` and `DiGraphFrame`, as well as
`CallFrame`, which owns the function-call boundary. Additionally, dialects may
define their own frames, such as `kirin-scf`’s `ScfIfFrame`. All of these frame
structs implement the `Frame` trait, describing how the program should be
traversed. Which frames belong to the engine and which a dialect gets to define
turns out to be a meaningful distinction, and we return to it below.

A frame therefore sits between two effect algebras, and translating between them
is most of what its methods do:

|  | produced by | consumed by | vocabulary |
| --- | --- | --- | --- |
| `SparseForwardEffect` | a dialect rule | the frame walking that statement | the program — jump, call, return, yield |
| `FrameEffect` | a frame | `drive_frames` | the frame stack — continue, push, done, complete |
| `Completion` | a frame that finished | its parent frame | how a body ended — returned, yielded, finished |

#### `SparseForwardEffect`

We saw in [`Interp`](#interp-trait) that `ConcreteInterpreter` sets `Effect =
SparseForwardEffect<V, FrameStackItem<V, E>>`. This is the algebra a dialect
rule produces when one of its statements is interpreted:

```rust
pub enum SparseForwardEffect<V, F> {
    /// Statement done; continue with the next statement.
    Next,
    /// Unconditional transfer to a block in the current CFG.
    Jump(Edge<V>),
    /// Conditional transfer whose condition is undecided in the value domain.
    Branch(Vec<Edge<V>>),
    /// Invoke a function through the engine's `Linker`.
    Call(CallEffect<V>),
    /// Terminate the innermost enclosing body block with carried values.
    Yield(Product<V>),
    /// Return from the enclosing function.
    Return(Product<V>),
    /// Run a sub-computation by pushing a dialect-owned `frame`; when it
    /// finishes, its values land in `results`.
    Push { frame: F, results: Product<SSAValue> },
}
```

A `FrameEffect` describes what to do with the frame stack. In contrast, a
`SparseForwardEffect` describes what a *statement* wants in terms of the
program: jump to this block, invoke that callee, return these values. A dialect
rule produces one of these and nothing more. The frame that receives the effect
is the one that decides what what should be done with the effect.

We can think of this process as follows:

1. The dialect defines rules (through the `Interpretable` trait) describing what
   the interpreter should do with a given operation. These rules produce a
   `SparseForwardEffect` describing how the program’s traversal should continue.
   For example:
    * An arithmetic operation will just return a `Next` effect
    * An `if` operation will return a `Push` effect with the block that the
       condition selects.
    * A `cf.ConditionalBranch` will return a `Jump` effect when its condition
       is decided, and a `Branch` effect if its condition is undecided. In
       practice for `ConcreteInterpreter`, the condition will always be decided.
       But there may be other interpreters/analysis where this may not be the
       case.
2. The interpreter’s frame engine executes the current frame which calls the
   current statement’s rule and receives the corresponding
   `SparseForwardEffect`. This is done within the frame’s `step_into` function,
   which then takes the `SparseForwardEffect` and translates it to a
   `FrameEffect`.
3. The `drive_frames` function finally takes the `FrameEffect` and decides what
   to do with the frame stack. This process loops until the end of the program
   is reached.

#### Engine frames and dialect frames

Frames come from two places, and the rule for which is simple: a frame lives
where the decision it makes is defined.

|  | engine frames | dialect frames |
| --- | --- | --- |
| examples | `BlockFrame`, `CFGFrame`, `DiGraphFrame`, `CallFrame` | `ScfIfFrame`, `ScfForFrame` |
| encodes | body representation mechanics, runtime bookkeeping | what an operation means |
| constructed by | the engine, or a parent frame | the dialect rule, through a dispatch trait |
| varies per engine | no — one walker per body kind | yes — concrete, abstract and dense variants |
| engine capabilities needed | whatever the job requires (`CallFrame`: `CallServices + Env`) | as few as possible (`ScfIfFrame`: only `FrameEngine`) |

The frames defined with the engine encode things no dialect gets to define:

- `BlockFrame`, `CFGFrame` and `DiGraphFrame` are *representation walkers*, one
  per variant of a `Body` defined in the IR. That a block is a linear statement
  sequence, that a CFG follows jump edges, and that a digraph runs in dependency
  order are facts about the representation, identical for every language, so
  there is exactly one walker each. (`UnGraph` has no default walker, since an
  undirected graph has no inherent execution order.)
- `CallFrame` can be thought of as interpreter runtime bookkeeping. It holds the
  callee’s `EnvIndex` and where its returned values must go. Despite the name,
  it is not a `kirin-function` operation. The calling convention is written
  entirely in the interpreter framework’s vocabulary (`Callee`, `Linker`,
  `ResolvedCallable`, `Body`, `EnvIndex`). The root call pushed by
  `ConcreteInterpreter::call` has no statement behind it at all, any dialect may
  issue a `Call` effect, and freeing the callee activation exactly once is an
  invariant worth having a single implementation of.

Dialect frames, by contrast, encode what an *operation means*. Which arm of
`scf.if` runs, and the fact that the arm’s yielded values become the operation’s
results, is the definition of `scf.if`; the loop-carried fixpoint is the
definition of `scf.for`.

The test to apply is whether there is a decision left over once a rule has
produced its effect. `scf.if` knows something the framework cannot: which arm to
enter, so it must supply a frame. A call statement knows only callee identity,
arguments and result slots, and once it has emitted them nothing is left to
decide. This distinction shows up in the type bounds. `ScfIfFrame`’s `Frame`
implementation requires only `I: FrameEngine<Error = E>`, requiring no engine
capability whatsoever, because the rule already read the condition and stored
the decision in the frame. `CallFrame` is the opposite, as it requires the
interpreter engine to implement `CallServices + Env<Anchor = SSAValue>`
capabilities. Otherwise, the engine would have no means of handling a
`CallFrame`.

It also shows up in how they vary per engine. There is one `BlockFrame`, but
`ScfIfFrame`, `AbstractScfIfFrame` and `DenseScfIfFrame` are implemented once
per engine type. What varies there is the *semantic policy*, i.e. whether to
pick the decided arm, or explore both arms and join their results. This semantic
policy is described by the dialect, whereas traversal mechanics need no such
split.

So when the compiler developer uses dialects with their own frames and wants to
use `ConcreteInterpreter`, they need to compose the engine defined frames with
the dialect defined frames as follows:

```rust
#[derive(Frame)]
pub(crate) enum MyLanguageFrames<V, E> {
    Block(BlockFrame<V, E>),
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
    ScfIf(ScfIfFrame<V, E>),
    ScfFor(ScfForFrame<V, E>),
}
```

### Putting it together: `SparseForwardInterp`

An interpreter that implements an `Env` anchored on SSA-values and uses the
`SparseForwardEffect` algebra is considered to be a `SparseForwardInterp`:

```rust
pub trait SparseForwardInterp:
    Env<Anchor = SSAValue> + Interp<Effect = SparseForwardEffect<<Self as Interp>::Value, Self::Frame>>
{
    type Frame;
}

impl<V, F, I> SparseForwardInterp for I
where
    I: Env<Anchor = SSAValue> + Interp<Value = V, Effect = SparseForwardEffect<V, F>>,
    I::Semantics: SparseForwardSemantic,
{
    type Frame = F;
}
```

The blanket implementation above means that `ConcreteInterpreter` doesn’t have
to implement `SparseForwardInterp` explicitly. Moreover, by binding

```rust
Effect = SparseForwardEffect<<Self as Interp>::Value, Self::Frame>
```

Any dialect rule acting on a `SparseForwardInterp`, knows that it should produce
a `SparseForwardEffect`.

`SparseForwardInterp` also includes the following helper methods which any
dialect rules can access:

```rust
    /// Read one SSA value from the current activation.
    fn read(&self, value: impl Into<SSAValue>) -> Result<Self::Value, Self::Error> {
        self.env_read(self.index(), value.into())
    }

    /// Read a list of SSA values into a [`Product`].
    fn read_many(&self, values: &[SSAValue]) -> Result<Product<Self::Value>, Self::Error> {
        values.iter().map(|value| self.read(*value)).collect()
    }

    /// Write one SSA value into the current activation.
    fn write(&mut self, value: impl Into<SSAValue>, data: Self::Value) -> Result<(), Self::Error> {
        let index = self.index();
        self.env_write(index, value.into(), data)
    }

    /// Destructure a [`Product`] into result slots, checking arity.
    fn write_results<T: Into<SSAValue> + Copy>(
        &mut self,
        values: &[T],
        data: Product<Self::Value>,
    ) -> Result<(), Self::Error> {
        if values.len() != data.len() {
            return Err(Self::Error::from(InterpreterError::ProductArityMismatch {
                expected: values.len(),
                actual: data.len(),
            }));
        }
        for (value, data) in values.iter().zip(data) {
            self.write(*value, data)?;
        }
        Ok(())
    }
```

Essentially, `SparseForwardInterp` is a common flavor of interpreter (i.e.
implemented by `ConcreteInterpreter`) that dialects can define rules upon to
remove the need of specifying individually that this rule’s interpreter has an
`Env` and pushes `SparseForwardEffect`s.

### Running the Interpreter

Finally, to actually use a `ConcreteInterpreter`, the compiler writer will
create an object of the `ConcreteInterpreter` struct, defining the open type
parameters. The following is the example from the `example/toy-lang` crate:

```rust
pub type ToyInterpreter<'ir, Lk = CrossStageLinker> = ConcreteInterpreter<
    'ir,
    Stage,                  // an enum defining the compiler's stages
    i64,                    // The type of values produced by the language
    ToyError,               // Some error type
    Lk,                     // A linker
    ToyFrame<i64, ToyError> // A frame enum composing engine and dialect frames
>;

let mut interp: ToyInterpreter<'_> =
    ConcreteInterpreter::new(pipeline).with_linker(CrossStageLinker);
```

Here, the `pipeline` object contains the source code that we want to execute. To
run this code, it is simply a matter of calling `ConcreteInterpreter`'s
`call_by_name` method:

```rust
interp.call_by_name(stage_name, function_name, args.iter().copied())
```

where `stage_name` and `function_name` are the names of the stage and function
in the code that we want to run, and `args` are the arguments to be passed to
this function.

## Dialect Rules

Now that we have explained the mechanisms and abstractions behind an interpreter
or analysis in `kirin`, we should look into how a dialect author interfaces with
these to provide meaning to the operations they define. This is done through the
dialect rules, which we have seen come up already.

A dialect rule is an implementation of the `Interpretable` trait by a dialect
defined op. This trait is defined as follows:

```rust
pub trait Interpretable<I: Interp, Semantics>: Dialect {
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error>;
}
```

Given some interpreter `interp`, a dialect rule processes the operation and
produces some kind of effect. If the interpreter is a `ConcreteInterpreter` and
the operation is an arithmetic operation, the rule will write the result of the
arithmetic operation into the interpreter’s environment (see [`write` for
`SparseForwardInterp`](#putting-it-together-sparseforwardinterp)) and then
return a `SparseForwardEffect::Next`.

The `Interpretable` abstraction allows use to define a different rule for
different interpreters. If our interpreter were instead a liveness analysis,
then the rule will mark the arithmetic op’s uses as live and kill the value the
op is bound to.

## Constant Propagation

Another interpreter that is implemented in Kirin is a constant propagation
interpreter. This is implemented in the `kirin-constprop` sub-crate. When we
take a look at this crate, we see that it implements a type alias:

```rust
pub type ConstProp<'ir, S, E, Lk = kirin_interpreter::SameStageLinker> =
    kirin_interpreter::SparseForwardInterpreter<'ir, S, ConstPropValue, E, Lk, ConstPropContext>;
```

We see that the `ConstProp` interpreter engine is a specialization of a
`SparseForwardInterpreter`.

Be careful: `SparseForwardInterpreter` ≠ `SparseForwardInterp`!

`SparseForwardInterpreter` is a struct describing the forward
abstract-interpretation engine, whereas `SparseForwardInterp` is a trait
describing a set of capabilities that a dialect rule can use. So the
`Interpreter` is what the compiler-author calls to run an analysis, and the
`Interp` is what the dialect-author implements to define the semantic rules for
their operations.

More concretely, the author of an arithmetic operation dialect will implement
the [`Interpretable` trait](#dialect-rules) with `I: SparseForwardInterp` for
each of the arithmetic operations.  These dialect rules only need to be
implemented once to be used by both a `ConcreteInterpreter` and a `ConstProp`.
The compiler-author, if they want to use a `ConcreteInterpreter` or a
`ConstProp` struct, will instantiate such objects. They can then use the `call`
and `analyze` methods respectively to run the two interpreters without needing
to implement any of the underlying mechanism.

### SparseForwardInterpreter

Unlike `ConcreteInterpreter` which uses the standard `drive_frames` function in
its execution, when `SparseForwardInterpreter::analyze` is called, it uses an
internal driver that underpins its analysis mechanism.

```rust
pub struct SparseForwardInterpreter<
    'ir,
    S: StageMeta,
    V,
    E,
    Lk = SameStageLinker,
    P = ContextInsensitive,
    F = StandardAbstractFrame<V, E, <P as CallContext<V>>::Key>,
    Sem = ForwardEval,
> where
    V: Clone + HasBottom,
    E: From<InterpreterError>,
    P: CallContext<V>,
    Sem: SparseForwardSemantic,
{
    driver: ForwardDriver<'ir, S, V, E, Lk, P, F, Sem>,
}
```

It is this `ForwardDriver` that implements the fixpoint analysis mechanism that
runs constant propagation until there are no more changes in the analysis:

```rust
type ForwardDriver<'ir, S, V, E, Lk, P, F, Sem> = StandardFixpointInterpreter<
    SparseForwardTransfer<'ir, S, V, E, Lk, P, F, Sem>,
    SparseForwardProfile<V, E, <P as CallContext<V>>::Key, F>,
    ForwardStore<<P as CallContext<V>>::Key, V>,
    ForwardSummaryDeps<Owner<<P as CallContext<V>>::Key>>,
>;
```

Without delving into the details, the forward driver is the mechanism that
traverses the IR and computes the given analysis at each SSA-value. Remember,
these values are computed from [dialect rules](#dialect-rules). It will do this
until the analysis no longer updates and reaches a fixed point.

`ConstProp` doesn’t need to re-implement any of this mechanism. Instead, it
defines the lattice of values (`ConstPropValue`) that the constant propagation
analysis can compute, including the `join` and `meet` methods used to merge two
such `ConstPropValue`s.

```rust
pub enum ConstPropValue<C = i64, S = String, F = String> {
    Bottom,
    Const(C),
    PartialTuple(Box<PartialTuple<Self>>),
    PartialStruct(Box<PartialStruct<S, F, Self>>),
    Top,
}
```

It also defines functions for converting from other value kinds (e.g.
`ArithValue` to `Const` or `Top` if not `i64`) that are produced by the dialect
rules.

Additionally, `SparseForwardInterpreter`'s default `CallContext` is
context-insensitive meaning that every call site of a function shares the same
analysis value. We would like the constant propagation to be more granular, and
compute distinct analysis values depending on the arguments passed at each call
site. For this purpose, the `CallCtx` and `ConstPropContext` types are defined.

For further details, take a look at the [ConstProp Framework documentation](../design/constprop-framework.md).
