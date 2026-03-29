# Seed & Execute

A seed is a bundle of an IR entry point and runtime values that can kick-start interpretation.
Seeds give dialect authors full `&mut I` access for complex execution patterns that can't be
expressed as a single effect return.

## Execute Trait

```rust
trait Execute<I: Machine> {
    fn execute(self, interp: &mut I) -> Result<I::Effect, I::Error>;
}
```

- **`Self` is the seed type**, `I` is the interpreter.
- **Returns `I::Effect`** — the terminal effect from execution (Return, Yield, Jump, etc.).
- **`self` is consumed** — seeds carry arguments bound during execution.
- **Most impls are generic over `I`** — executing a block is the same for most interpreters.

## Built-in Seed Types

- `BlockSeed<V>` — Block + block args. Bind args, step through statements, return terminator's effect.
- `RegionSeed<V>` — Region + region args. Dispatch to entry block, follow control flow.
- `FunctionSeed<V>` — SpecializedFunction + args + result slots. Push frame, run, handle return.
- `StagedFunctionSeed<V>` — StagedFunction + args. Resolve stage, then delegate to FunctionSeed.

## Dialect-Defined Seeds

- `ZXGraphSeed` — UnGraph + port args + captures. Execute a ZX diagram.
- `ComputeGraphSeed` — DiGraph + port args + captures. Execute a computational graph.
- `ForLoopSeed<V>` — loop bounds + body block + init args. Execute an SCF for loop.

## Terminal Effects

A seed returns the terminal effect from its execution. The caller matches on it:

- `Yield(V)` — in SCF context (scf.yield terminates the block)
- `Jump(Block, Args)` — in CF context (cf.branch terminates the block)
- `Return(V)` — in function context (function.return terminates the body)

Low-level seeds (`BlockSeed`) return the raw terminal effect — building blocks.
High-level seeds (`ForLoopSeed`, `FunctionSeed`) compose low-level seeds and handle
terminal effects internally. See [examples.md](examples.md) for full examples.

## Seed Composition

The top-level seed type is an enum of all possible seeds:

```rust
enum CompositeSeed<V> {
    Block(BlockSeed<V>),
    Region(RegionSeed<V>),
    Function(FunctionSeed<V>),
}

impl<I: Machine> Execute<I> for CompositeSeed<V>
where
    BlockSeed<V>: Execute<I>,
    FunctionSeed<V>: Execute<I>,
{
    fn execute(self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            Self::Block(s) => s.execute(interp),
            Self::Function(s) => s.execute(interp),
            // ...
        }
    }
}
```
