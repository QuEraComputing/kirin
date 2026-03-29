# Example 4: SCF If (Custom Seed)

Structured control flow operations define custom seeds that compose `BlockSeed`. The
`IfSeed` executes one of two blocks and handles the Yield terminal effect.

This is the pattern used by `kirin-scf`.

## Key Characteristics

- The dialect's `interpret()` creates a seed and executes it via `&mut I`
- The seed creates a `BlockSeed` and calls `.execute(interp)` to run a block
- Pattern-matches on the terminal to extract the yielded value

## Dialect Code (If operation)

```rust
impl<I: Interpreter> Interpretable<I> for If<T>
where
    I::Value: BranchCondition + ProductValue,
    IfSeed: Execute<I>,
{
    type Effect = ();
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, I::Seed, ()>, InterpError<Infallible>>
    {
        let cond = interp.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => return Err(InterpreterError::unsupported("nondeterministic scf.if").into()),
        };

        IfSeed {
            block,
            results: self.results.clone(),
        }.execute(interp)?;

        Ok(Effect::Advance)
    }
}
```

## Seed Code (IfSeed)

```rust
struct IfSeed {
    block: Block,
    results: Product<ResultValue>,
}

impl<I: Interpreter> Execute<I> for IfSeed
where
    I::Value: ProductValue,
    BlockSeed<I::Value>: Execute<I>,
{
    fn execute(self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let terminal = BlockSeed::entry(self.block).execute(interp)?;

        // Match the terminal effect — expect Yield from scf body
        match terminal {
            Effect::Yield(v) => {
                Ok(Effect::BindProduct(self.results, v))
            }
            _ => Err(InterpreterError::unsupported("expected yield from scf.if body").into()),
        }
    }
}
```

## How It Works

1. `interpret()` reads the condition, picks the block, creates `IfSeed`, calls `.execute(interp)`
2. `IfSeed::execute()` creates a `BlockSeed` and calls `.execute(interp)` which runs the block and returns the terminal effect
3. The terminator is `scf.yield` → produces `Effect::Yield(v)`
4. The seed matches `Yield(v)`, returns `BindProduct` to write the result to SSA slots
5. Back in `interpret()`, returns `Advance` to move to the next statement
