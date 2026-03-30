# Example 5: SCF For Loop (Inline Loop)

`scf.for` also does not automatically justify its own seed type. The loop orchestration can
stay inline in `interpret()` while reusing `BlockSeed` for the body.

## Key Characteristics

- The operation owns the loop-specific control logic inline
- Each iteration executes the body through `BlockSeed`
- The returned `Yield` becomes the carried state for the next iteration
- No `ForLoopSeed` is needed unless the same loop kernel is reused elsewhere

## Code

```rust
impl<I: Interpreter> Interpretable<I> for For<T>
where
    I::Value: ForLoopValue + ProductValue,
    BlockSeed<I::Value>: Execute<I, Output = I::Effect>,
{
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, Infallible>, InterpError<Infallible>>
    {
        let mut iv = interp.read(self.start)?;
        let end = interp.read(self.end)?;
        let step = interp.read(self.step)?;
        let mut carried = interp.read(self.init)?;

        while iv.loop_condition(&end) == Some(true) {
            let args = smallvec![iv.clone(), carried];
            let terminal = BlockSeed::new(self.body, args).execute(interp)?;

            match terminal {
                Effect::Yield(value) => {
                    carried = value;
                }
                _ => {
                    return Err(
                        InterpreterError::unsupported("expected yield from for body").into()
                    );
                }
            }

            iv = iv.loop_step(&step).ok_or_else(|| {
                InterpError::from(InterpreterError::message("induction variable overflow"))
            })?;
        }

        Ok(Effect::BindProduct(self.results.clone(), carried).then(Effect::Advance))
    }
}
```

## Why There Is No `ForLoopSeed`

The loop is imperative, but that does not by itself justify a separate abstraction:

- the orchestration belongs to `scf.for`
- the reusable part is already `BlockSeed`
- introducing `ForLoopSeed` would mostly move code out of `interpret()` without defining a
  stable reusable entrypoint

If multiple operations later share the same loop executor, the design can revisit this.

## Execution Flow

1. Read loop bounds and initial carried state
2. While the induction variable says to continue:
3. Execute the loop body via `BlockSeed`
4. Match `Yield(value)` and thread it into the next iteration
5. Step the induction variable
6. After the loop, return `BindProduct(...).then(Advance)`
