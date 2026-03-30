# Example 9: Composed Dialect (Mixed Effects)

When some sub-dialects have machine effects and others don't. This requires Lift impls
for the composed effect type.

## Key Characteristics

- Sub-dialects have different `Effect` types (`Infallible` vs `MemoryEffect`)
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

impl Lift<Infallible> for MixedEffect {
    fn lift(from: Infallible) -> Self {
        match from {}
    }
}

impl<I: Interpreter> Interpretable<I> for MixedLanguage<T>
where
    Add<T>: Interpretable<I, Effect = Infallible, Error = Infallible>,
    MemoryBarrier<T>: Interpretable<I, Effect = MemoryEffect, Error = Infallible>,
{
    type Effect = MixedEffect;
    type Error = Infallible;

    fn interpret(&self, interp: &mut I)
        -> Result<Effect<I::Value, MixedEffect>, InterpError<Infallible>>
    {
        match self {
            Self::Add(op) => Ok(Lift::lift(op.interpret(interp)?)),
            Self::Barrier(op) => Ok(Lift::lift(op.interpret(interp)?)),
        }
    }
}
```

## How Lift Works Here

`Lift<Effect<V, DEA>> for Effect<V, DEC>` where `DEC: Lift<DEA>`:

For each match arm:

- **Add** returns `Effect<V, Infallible>`. `Lift::lift` converts to `Effect<V, MixedEffect>`:
  - `Advance` → `Advance` (pass-through)
  - `BindValue(s, v)` → `BindValue(s, v)` (pass-through)
  - `Machine(_)` is impossible because `Infallible` is uninhabited

- **Barrier** returns `Effect<V, MemoryEffect>`. `Lift::lift` converts to `Effect<V, MixedEffect>`:
  - `Machine(MemoryEffect::Barrier)` → `Machine(MixedEffect::Memory(Barrier))` via `Lift::lift`
  - All other variants pass through unchanged

## The `Lift<Infallible>` Impl

`Lift<Infallible> for MixedEffect` is needed because the
`Lift<Effect<V, Infallible>> for Effect<V, MixedEffect>` impl requires
`MixedEffect: Lift<Infallible>`. Unlike `()`, `Infallible` makes the impossible case explicit:
the `Machine(_)` branch for the pure dialect cannot exist.
