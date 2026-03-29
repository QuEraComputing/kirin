# Example 5: SCF For Loop (Seed Composition)

The most complex seed example — a loop that repeatedly executes a block, using the
yielded value as carried state for the next iteration.

## Key Characteristics

- `ForLoopSeed` composes `BlockSeed` in a loop
- Each iteration matches on `Yield(v)` to extract carried state
- The final carried state is bound to result SSA slots
- Demonstrates seeds as the escape hatch for complex execution patterns

## Code

```rust
struct ForLoopSeed<V> {
    start: V,
    end: V,
    step: V,
    body: Block,
    init: V,
    results: Product<ResultValue>,
}

impl<I: Interpreter> Execute<I> for ForLoopSeed<I::Value>
where
    I::Value: ForLoopValue + ProductValue,
    BlockSeed<I::Value>: Execute<I>,
{
    fn execute(self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let mut iv = self.start;
        let mut carried = self.init;

        while iv.loop_condition(&self.end) == Some(true) {
            let args = smallvec![iv.clone(), carried];
            let terminal = BlockSeed::new(self.body, args).execute(interp)?;

            match terminal {
                Effect::Yield(v) => { carried = v; }
                _ => return Err(
                    InterpreterError::unsupported("expected yield from for body").into()
                ),
            }

            iv = iv.loop_step(&self.step).ok_or_else(|| {
                InterpError::from(InterpreterError::message("induction variable overflow"))
            })?;
        }

        Ok(Effect::BindProduct(self.results, carried).then(Effect::Advance))
    }
}
```

## Execution Flow

1. Initialize induction variable `iv` and carried state from seed fields
2. Loop while `iv < end`:
   a. Pack `[iv, carried]` as block args
   b. Create `BlockSeed`, call `.execute(interp)` → runs the for body
   c. Match terminal: expect `Yield(v)` → update carried state
   d. Step the induction variable
3. After loop: bind final carried state to results, advance cursor

## Why This Is a Seed, Not an Effect

The for loop needs to orchestrate multiple block executions in a loop, using return
values from each iteration. This can't be expressed as a single returned effect — it
requires imperative control flow with `&mut I` access. Seeds are the designed escape
hatch for this pattern.

Compare with the dialect's `interpret()`, which just creates the seed and delegates:

```rust
fn interpret(&self, interp: &mut I)
    -> Result<Effect<I::Value, I::Seed, ()>, InterpError<Infallible>>
{
    // Read loop bounds, create ForLoopSeed, execute it
    ForLoopSeed { start, end, step, body, init, results }.execute(interp)?;
    Ok(Effect::Advance)
}
```
