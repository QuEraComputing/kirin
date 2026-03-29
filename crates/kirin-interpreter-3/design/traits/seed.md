# Seed & Execute

A seed is a bundle of an IR entry point and runtime values that can kick-start interpretation.
Seeds give dialect authors full `&mut I` access for complex execution patterns that can't be
expressed as a single effect return.

## Execute Trait

```rust
trait Execute<I: Interpreter> {
    fn execute(self, interp: &mut I) -> Result<I::Effect, I::Error>;
}
```

- **`Self` is the seed type**, `I` is the interpreter.
- **`I: Interpreter`** — seeds use `ValueRead`, `PipelineAccess`, and `consume_effect`.
- **Returns `I::Effect`** — the concrete `Effect<I::Value, I::Seed, I::DialectEffect>`. This is
  the terminal effect from execution (Return, Yield, Jump, etc.).
- **`self` is consumed** — seeds carry arguments bound during execution.
- **Most impls are generic over `I`** — executing a block is the same for most interpreters.

## Built-in Seed Types

- `BlockSeed<V>` — Block + block args. Bind args, step through statements, return terminator's effect.
- `RegionSeed<V>` — Region + region args. Dispatch to entry block, follow control flow.
- `FunctionSeed<V>` — SpecializedFunction + args + result slots. Push frame, run, handle return.
- `StagedFunctionSeed<V>` — StagedFunction + args. Resolve stage, then delegate to FunctionSeed.

## Dialect-Defined Seeds

- `IfSeed` — condition block + result slots. Execute one block, match `Yield`.
- `ForLoopSeed<V>` — loop bounds + body block + init args. Execute an SCF for loop.
- `ZXGraphSeed` — UnGraph + port args + captures. Execute a ZX diagram.
- `ComputeGraphSeed` — DiGraph + port args + captures. Execute a computational graph.

## Terminal Effects

A seed returns the terminal effect from its execution. The caller matches on it:

- `Effect::Yield(V)` — in SCF context (scf.yield terminates the block)
- `Effect::Jump(Block, Args)` — in CF context (cf.branch terminates the block)
- `Effect::Return(V)` — in function context (function.return terminates the body)

Low-level seeds (`BlockSeed`) return the raw terminal effect — building blocks.
High-level seeds (`ForLoopSeed`, `FunctionSeed`) compose low-level seeds and handle
terminal effects internally. See [examples](../examples/index.md) for full examples.

## How Seeds Compose

Seeds compose via `Execute`. A high-level seed like `IfSeed` creates a `BlockSeed` and
calls `.execute(interp)` on it:

```rust
impl<I: Interpreter> Execute<I> for IfSeed
where
    I::Value: ProductValue,
    BlockSeed<I::Value>: Execute<I>,
{
    fn execute(self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let terminal = BlockSeed::entry(self.block).execute(interp)?;

        match terminal {
            Effect::Yield(v) => Ok(Effect::BindProduct(self.results, v)),
            _ => Err(InterpreterError::unsupported("expected yield from scf.if body").into()),
        }
    }
}
```

## Seed Composition

The top-level seed type is an enum of all possible seeds:

```rust
enum CompositeSeed<V> {
    Block(BlockSeed<V>),
    Region(RegionSeed<V>),
    Function(FunctionSeed<V>),
    StagedFunction(StagedFunctionSeed<V>),
}

impl<I: Interpreter> Execute<I> for CompositeSeed<I::Value>
where
    BlockSeed<I::Value>: Execute<I>,
    RegionSeed<I::Value>: Execute<I>,
    FunctionSeed<I::Value>: Execute<I>,
    StagedFunctionSeed<I::Value>: Execute<I>,
{
    fn execute(self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            Self::Block(s) => s.execute(interp),
            Self::Region(s) => s.execute(interp),
            Self::Function(s) => s.execute(interp),
            Self::StagedFunction(s) => s.execute(interp),
        }
    }
}
```

## Why Seeds vs Effects

An earlier design had the dialect return `Execute(FunctionSeed)` as an effect. This was
dropped because:

1. **Orphan rule**: Dialect-defined seeds can't implement `Lift` for interpreter-defined effect
   types — neither crate owns both types.
2. **Unnecessary indirection**: With `&mut I` access, the dialect can execute the seed directly.
3. **Simpler API**: The dialect returns `Effect::Advance` after the seed completes. No
   custom effect type to define, no Lift to implement.
