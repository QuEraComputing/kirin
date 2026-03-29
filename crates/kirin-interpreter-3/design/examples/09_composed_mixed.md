# Example 9: Composed Dialect (Mixed Effects)

When some sub-dialects have machine effects and others don't. This requires Lift impls
for the composed effect type.

## Key Characteristics

- Sub-dialects have different `Effect` types (`()` vs `MemoryEffect`)
- The composed dialect defines a sum type for all machine effects
- `Lift::lift` converts between `Effect<V, S, SubDE>` and `Effect<V, S, ComposedDE>`
- Only the `Machine(de)` variant is transformed; all others pass through

## Code

```rust
#[derive(Dialect)]
enum MixedLanguage<T> {
    Add(Add<T>),
    Barrier(MemoryBarrier<T>),
}

// Composed machine effect — sum of all sub-dialect machine effects
enum MixedEffect {
    Memory(MemoryEffect),
}

// Lift impls: sub-dialect effects → composed effect
impl Lift<MemoryEffect> for MixedEffect {
    fn lift(from: MemoryEffect) -> Self { Self::Memory(from) }
}

impl Lift<()> for MixedEffect {
    fn lift(_: ()) -> Self { unreachable!("() means no machine effects") }
}

impl<I: Interpreter> Interpretable<I> for MixedLanguage<T>
where
    Add<T>: Interpretable<I, Effect = (), Error = Infallible>,
    MemoryBarrier<T>: Interpretable<I, Effect = MemoryEffect, Error = Infallible>,
{
    type Effect = MixedEffect;
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, I::Seed, MixedEffect>, InterpError<Infallible>>
    {
        match self {
            Self::Add(op) => Ok(Lift::lift(op.interpret(interp)?)),
            Self::Barrier(op) => Ok(Lift::lift(op.interpret(interp)?)),
        }
    }
}
```

## How Lift Works Here

`Lift<Effect<V, S, DEA>> for Effect<V, S, DEC>` where `DEC: Lift<DEA>`:

For each match arm:

- **Add** returns `Effect<V, S, ()>`. `Lift::lift` converts to `Effect<V, S, MixedEffect>`:
  - `Advance` → `Advance` (pass-through)
  - `BindValue(s, v)` → `BindValue(s, v)` (pass-through)
  - `Machine(())` → unreachable (Add never produces machine effects)

- **Barrier** returns `Effect<V, S, MemoryEffect>`. `Lift::lift` converts to `Effect<V, S, MixedEffect>`:
  - `Machine(MemoryEffect::Barrier)` → `Machine(MixedEffect::Memory(Barrier))` via `Lift::lift`
  - All other variants pass through unchanged

## The Lift<()> Impl

`Lift<()> for MixedEffect` is needed because the `Lift<Effect<V, S, ()>> for Effect<V, S, MixedEffect>`
impl requires `MixedEffect: Lift<()>`. Since `Add` has `Effect = ()`, the composed type must be
able to lift `()`. The impl is unreachable at runtime — Add never produces `Machine(())`
variants — but the type system requires it.
