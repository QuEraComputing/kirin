# Example 9: Composed Dialect (Mixed Effects)

When some sub-dialects have machine effects and others don't. This requires Lift impls
for the composed effect type and `try_lift()` between GAT instantiations.

## Key Characteristics

- Sub-dialects have different `Effect` types (`()` vs `MemoryEffect`)
- The composed dialect defines a sum type for all machine effects
- `try_lift()` converts between `I::Effect<SubDE>` and `I::Effect<ComposedDE>`
- Framework-provided `Lift<I::Effect<DE>> for I::Effect<DE2>` when `DE2: Lift<DE>`

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

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<MixedEffect>, I::Error<Infallible>> {
        match self {
            Self::Add(op) => op.interpret(interp)?.try_lift(),
            Self::Barrier(op) => op.interpret(interp)?.try_lift(),
        }
    }
}
```

## How try_lift Works Here

The framework provides a blanket Lift between GAT instantiations:

```rust
impl Lift<I::Effect<DE>> for I::Effect<DE2> where DE2: Lift<DE>
```

For each match arm:

- **Add** returns `I::Effect<()>`. `try_lift()` converts to `I::Effect<MixedEffect>`:
  - `Base(base)` → `Base(base)` (pass-through)
  - `Execute(seed)` → `Execute(seed)` (pass-through)
  - `Machine(())` → unreachable (Add never produces machine effects)

- **Barrier** returns `I::Effect<MemoryEffect>`. `try_lift()` converts to `I::Effect<MixedEffect>`:
  - `Base(base)` → `Base(base)` (pass-through)
  - `Machine(MemoryEffect::Barrier)` → `Machine(MixedEffect::Memory(Barrier))` via `Lift::lift`

## The Lift<()> Impl

`Lift<()> for MixedEffect` is needed because the framework Lift between GAT instantiations
requires `MixedEffect: Lift<()>`. Since `Add` has `Effect = ()`, the composed type must be
able to lift `()`. The impl is unreachable at runtime — Add never produces `Machine(())`
variants — but the type system requires it.
