# Machine Kernel

## Kernel Responsibilities

The framework kernel owns only the parts of execution that are genuinely
generic:

- the driver loop
- the current execution location
- the internal cursor stack
- breakpoint and fuel handling
- invoking statement semantics on the current statement
- consuming returned language effects
- applying minimal cursor-stack control actions

The kernel does not own language semantics such as:

- call frames
- return conventions
- yield conventions
- loop stacks
- graph traversal stacks
- product packing or unpacking policy

Those belong to dialect-defined state and effect types.

## Public Trait Family

The public surface should be centered on a thin machine trait plus statement
semantics:

```rust
trait Machine<'ir>: StageAccess<'ir> + RuntimeControl<'ir> {
    type State;
    type Error;
    type Stop;

    fn state(&self) -> &Self::State;
    fn state_mut(&mut self) -> &mut Self::State;
}
```

`Machine<'ir>` is intentionally thin:

- it is the shared composition root for interpreter shells
- it does not imply `ValueStore`
- it does not imply one universal language effect type

The primary semantics trait is:

```rust
trait Interpretable<'ir, I>: Dialect
where
    I: Machine<'ir>,
{
    type Effect: ConsumeEffect<'ir, I>;

    fn interpret<L>(&self, interp: &mut I) -> Result<Self::Effect, I::Error>
    where
        L: Interpretable<'ir, I> + 'ir;
}
```

The effect-consumption trait is:

```rust
trait ConsumeEffect<'ir, I>
where
    I: Machine<'ir>,
{
    fn consume(self, interp: &mut I) -> Result<KernelAction<I::Stop>, I::Error>;
}
```

This split means:

- statement semantics define a language-owned effect type
- effect consumption mutates dialect-owned state
- the framework still controls cursor progression through `KernelAction`

## Kernel Action

`KernelAction` is the framework-owned control language for the cursor stack:

```rust
enum KernelAction<Stop> {
    Advance,
    Stay,
    Push(ExecutionSeed),
    Replace(ExecutionSeed),
    Pop,
    Stop(Stop),
}
```

Meaning:

- `Advance`
  Move to the next statement in the current execution context.
- `Stay`
  Keep the current cursor unchanged after semantic-state updates.
- `Push(seed)`
  Start nested execution by pushing a new execution context.
- `Replace(seed)`
  Replace the current execution context with a new one.
- `Pop`
  Finish the current execution context and resume its parent.
- `Stop(stop)`
  Stop execution for a semantic reason defined by the interpreter shell.

This is intentionally minimal.

Dialects may maintain richer semantic stacks in their own state, but the kernel
only sees the generic cursor-stack operations.

## Execution Seeds

The kernel keeps full cursors internal. Public code constructs execution seeds.

The seed surface should use named per-shape seed structs wrapped by one public
enum:

```rust
struct BlockSeed {
    body: Block,
}

struct RegionSeed {
    body: Region,
}

struct DiGraphSeed {
    body: DiGraph,
}

struct UnGraphSeed {
    body: UnGraph,
}

enum ExecutionSeed {
    Block(BlockSeed),
    Region(RegionSeed),
    DiGraph(DiGraphSeed),
    UnGraph(UnGraphSeed),
}
```

The framework may later extend this with multi-seed fan-out support, analogous
to the old `Fork`, but v1 should keep `KernelAction` single-seed.

## Internal Cursor Stack

The kernel owns a stack of execution cursors.

This is not a semantic call stack. It is only the generic nesting stack for
execution contexts. Dialects may keep semantic frame data in their own state if
they need it.

The split is:

- kernel cursor stack
  - where execution currently is
  - what nested execution contexts are active
- dialect-owned state
  - what that nesting means semantically

This allows dialects to define call stacks, graph traversal stacks, or loop
stacks without forcing one framework-wide frame model.

## Step Lifecycle

The kernel small-step cycle is:

1. resolve the current statement from the top cursor
2. invoke `Interpretable::interpret`
3. obtain a language-owned effect value
4. consume that effect through `ConsumeEffect`
5. obtain `KernelAction`
6. apply that action to the cursor stack

The dynamic driver loop layers breakpoints, fuel checks, and stop policy around
this cycle.

## Default Body Runners

Because statements own body execution semantics, body runners are helper
facilities, not semantic authorities.

The framework should provide explicit default helpers such as:

- `DefaultBlockRunner`
- `DefaultCFGRegionRunner`

These helpers are optional reusable execution strategies for statements that
want standard CFG behavior. They do not define the meaning of `Block` or
`Region` globally.

Future graph helpers should follow the same naming rule: explicit default
execution strategies, not universal graph semantics.
