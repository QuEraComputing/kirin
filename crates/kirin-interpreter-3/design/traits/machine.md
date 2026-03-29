# Machine & Effect

A `Machine` is a stateful object that evolves by consuming `Effect`s. Dialects do not mutate the machine
directly — instead they produce effects, and the machine updates itself according to the semantics of
each effect. This design is inspired by algebraic effects.

## Machine Trait

```rust
trait Machine {
    type Effect;
    type Error;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error>;
}
```

The caller is responsible for projecting/lifting effects into the machine's effect type before calling
`consume_effect`. The machine only knows about its own effect vocabulary.

## Two Levels of Machine

The `Machine` trait serves two levels:

1. **Dialect machines** process dialect-specific effects. Simple dialects (arithmetic, comparisons)
   use `()` as their machine. Stateful dialects (memory model, symbol table) define a concrete
   machine struct with `type Effect = DialectSpecificEffect`.

2. **The interpreter** is also a `Machine`. Its `Effect` is the unified `Effect<V, Seed, DE>` type
   (see [effects.md](effects.md)). It handles all effect variants — cursor control, value binding,
   seed execution — and delegates `Machine(de)` to the dialect machine.

## Dialect Machine Composition

Machines compose by **product** (state), effects by **sum** (language):

```rust
struct MachineC { a: MachineA, b: MachineB }

enum EffectC { A(EffectA), B(EffectB) }
enum ErrorC { A(ErrorA), B(ErrorB) }

impl Machine for MachineC {
    type Effect = EffectC;
    type Error = ErrorC;

    fn consume_effect(&mut self, effect: EffectC) -> Result<(), ErrorC> {
        match effect {
            EffectC::A(e) => self.a.consume_effect(e).map_err(Lift::lift),
            EffectC::B(e) => self.b.consume_effect(e).map_err(Lift::lift),
        }
    }
}
```

## Direct Mutation via ProjectMut

With `&mut I` access, dialects can mutate their machine directly via `ProjectMut`. Machine effects
are for cases where state changes need to go through the effect pipeline (e.g., deferred processing,
cross-dialect effects, or abstract interpretation where the interpreter processes effects differently).
