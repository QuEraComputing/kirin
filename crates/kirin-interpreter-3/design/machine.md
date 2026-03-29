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

## Composition

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

## Dialect Machines

Each dialect optionally defines its own machine. Simple dialects (like arithmetic) that have no
persistent state use `()` as their machine. Dialects with state (e.g., a memory model, a symbol
table) define a concrete machine struct.

With `&mut I` access, dialects can mutate their machine directly via `ProjectMut`. Machine effects
are for cases where state changes need to go through the effect pipeline (e.g., deferred processing,
cross-dialect effects, or abstract interpretation where the interpreter processes effects differently).
