# Example 7: Stateful Dialect (Machine Effects)

When the dialect needs state changes processed through the machine's `consume_effect`
pipeline rather than direct mutation. This is important for abstract interpretation
compatibility, deferred processing, or cross-dialect effect ordering.

## Key Characteristics

- `type Effect = MemoryEffect` — the dialect declares its own machine effect type
- The effect goes to the `Machine(DE)` slot via `try_lift()`
- The interpreter routes `Machine(de)` to `dialect_machine.consume_effect(de)`
- No `ProjectMut` needed — the machine handles its own state update

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

    fn interpret(&self, interp: &mut I) -> Result<I::Effect<MemoryEffect>, I::Error<Infallible>> {
        MemoryEffect::Barrier.try_lift()
    }
}
```

## How the Effect is Processed

1. `MemoryEffect::Barrier.try_lift()` → `I::Effect<MemoryEffect>::Machine(Barrier)`
2. Interpreter loop calls `self.consume_effect(effect)`
3. `consume_effect` matches `Machine(de)` → `self.dialect_machine.consume_effect(de)`
4. The composed `Machine` impl for the dialect machine handles `MemoryEffect::Barrier`

The dialect author only defines `MemoryEffect` and the `Machine` impl for their machine.
The framework routes the effect through the pipeline.

## Mixing Base and Machine Effects

A dialect can return either base effects or machine effects from the same impl using
`Seq` or conditional logic:

```rust
fn interpret(&self, interp: &mut I) -> Result<I::Effect<MemoryEffect>, I::Error<Infallible>> {
    if needs_barrier {
        // Machine effect
        MemoryEffect::Barrier.try_lift()
    } else {
        // Base effect (still valid — I::Effect<MemoryEffect> accepts BaseEffect too)
        BaseEffect::Advance.try_lift()
    }
}
```

Both paths use `try_lift()` — the `TryLift<BaseEffect<V>>` and `TryLift<MemoryEffect>` bounds
on `I::Effect<MemoryEffect>` handle routing to the correct slot.
