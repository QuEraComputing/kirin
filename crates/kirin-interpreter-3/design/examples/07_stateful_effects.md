# Example 7: Stateful Dialect (Machine Effects)

Stateful dialects must express semantic state changes through the machine's `consume_effect`
pipeline. This keeps the change visible to composition, ordering, and alternative
interpreter strategies.

## Key Characteristics

- `type Effect = MemoryEffect` — the dialect declares its own machine effect type
- The effect goes to the `Machine(DE)` variant of the unified `Effect` type
- The interpreter routes `Effect::Machine(de)` to `dialect_machine.consume_effect(de)`
- No direct mutation path — the machine handles its own state update

## Code

```rust
enum MemoryEffect {
    Flush,
    Barrier,
}

struct MemoryBarrier<T> {
    _phantom: PhantomData<T>,
}

impl<I: Interpreter> Interpretable<I> for MemoryBarrier<T> {
    type Effect = MemoryEffect;
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, MemoryEffect>, InterpError<Infallible>>
    {
        Ok(Effect::Machine(MemoryEffect::Barrier))
    }
}
```

## How the Effect is Processed

1. `Effect::Machine(MemoryEffect::Barrier)` is returned from `interpret()`
2. The execution loop lifts it: `Lift::lift(effect)` maps `Machine(Barrier)` to
   `Machine(ComposedDE::Memory(Barrier))` (if composed) or passes through (if top-level)
3. `consume_effect` matches `Effect::Machine(de)` → `self.dialect_machine.consume_effect(de)`
4. The composed `Machine` impl for the dialect machine handles `MemoryEffect::Barrier`

## Mixing Base and Machine Effects

A dialect can return either base effects or machine effects from the same impl using `Seq`:

```rust
fn interpret(&self, interp: &mut I)
    -> Result<Effect<I::Value, MemoryEffect>, InterpError<Infallible>>
{
    Ok(Effect::Seq(smallvec![
        Effect::BindValue(self.result, value),
        Effect::Machine(MemoryEffect::Flush),
        Effect::Advance,
    ]))
}
```

Because the unified `Effect` type contains all variants at the same level, `Seq` can
freely mix base effects and machine effects — no separate `BaseEffect` vs `SingleStageEffect`.
