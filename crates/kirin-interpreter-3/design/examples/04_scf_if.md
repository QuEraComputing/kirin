# Example 4: SCF If (Inline Orchestration)

Structured control flow operations usually do not need their own seed type. `scf.if`
can stay entirely inside `interpret()` while reusing the shared `BlockSeed` executor.

This is the pattern used by `kirin-scf`.

## Key Characteristics

- The operation reads its condition and chooses a block inline
- It reuses `BlockSeed` to execute the selected block
- It interprets the returned terminal inline
- No `IfSeed` is needed because the orchestration is specific to this operation

## Code

```rust
impl<I: Interpreter> Interpretable<I> for If<T>
where
    I::Value: BranchCondition + ProductValue,
    BlockSeed<I::Value>: Execute<I, Output = I::Effect>,
{
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, Infallible>, InterpError<Infallible>>
    {
        let cond = interp.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => return Err(InterpreterError::unsupported("nondeterministic scf.if").into()),
        };

        let terminal = BlockSeed::entry(block).execute(interp)?;

        match terminal {
            Effect::Yield(value) => {
                Ok(Effect::BindProduct(self.results.clone(), value).then(Effect::Advance))
            }
            _ => Err(InterpreterError::unsupported("expected yield from scf.if body").into()),
        }
    }
}
```

## Why There Is No `IfSeed`

`scf.if` does have imperative orchestration, but that alone is not enough to justify a seed:

- the logic is owned by one operation
- it starts from the current operation, not from a reusable IR entrypoint
- it can be expressed by calling the shared `BlockSeed` executor and interpreting the result

If another feature later needs the exact same control kernel, that would be the time to
introduce a reusable seed.

## Execution Flow

1. `interpret()` reads the condition and selects the block
2. `BlockSeed::entry(block).execute(interp)` runs that block and returns its terminal effect
3. `scf.if` matches on `Yield(value)`
4. `scf.if` returns `BindProduct(...).then(Advance)` as its ordinary effect
