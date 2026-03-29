# Example 4: SCF If (Custom Seed)

Structured control flow operations define custom seeds that compose `BlockSeed`. The
`IfSeed` executes one of two blocks and handles the Yield terminal effect.

This is the pattern used by `kirin-scf`.

## Key Characteristics

- The dialect's `interpret()` creates a seed and executes it via `&mut I`
- The seed composes `BlockSeed` and matches on the terminal effect
- `try_project()` extracts base effects from the terminal for pattern matching

## Dialect Code (If operation)

```rust
impl<I: Interpreter> Interpretable<I> for If<T>
where
    I::Value: BranchCondition + ProductValue,
    IfSeed<I::Value>: Execute<I>,
{
    type Effect = ();
    type Error = Infallible;

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<()>, I::Error<Infallible>> {
        let cond = interp.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => return InterpreterError::unsupported("nondeterministic scf.if").try_lift(),
        };

        IfSeed {
            block,
            results: self.results.clone(),
        }.execute(interp)?;

        BaseEffect::Advance.try_lift()
    }
}
```

## Seed Code (IfSeed)

```rust
struct IfSeed<V> {
    block: Block,
    results: Product<ResultValue>,
}

impl<I: Machine> Execute<I> for IfSeed<V>
where
    BlockSeed<V>: Execute<I>,
{
    fn execute(self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let terminal = BlockSeed::entry(self.block).execute(interp)?;

        // Match the terminal effect — expect Yield from scf body
        match terminal.try_project() {
            Ok(BaseEffect::Yield(v)) => {
                BaseEffect::BindProduct(self.results, v).try_lift()
            }
            _ => InterpreterError::unsupported("expected yield from scf.if body").try_lift(),
        }
    }
}
```

## How It Works

1. `interpret()` reads the condition, picks the block, creates `IfSeed`, calls `.execute(interp)`
2. `IfSeed::execute()` creates a `BlockSeed` for the chosen block and executes it
3. `BlockSeed::execute()` binds block args, steps through statements, returns the terminator's effect
4. The terminator is `scf.yield` → produces `BaseEffect::Yield(v)`
5. `try_project()` extracts the `BaseEffect` from the interpreter's effect type
6. The seed matches `Yield(v)`, binds the product to result SSA slots via `BindProduct`
7. Back in `interpret()`, returns `Advance` to move to the next statement
