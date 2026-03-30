# Machine

A `Machine` is a stateful effect consumer. Dialects do not mutate machine state directly;
they emit effects, and the relevant machine consumes them.

## Trait Definition

```rust
trait Machine {
    type Effect;
    type Error;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error>;
}
```

The caller is responsible for lifting a sub-dialect effect into the machine's effect type
before calling `consume_effect`.

## Two Levels of Machine

The same trait serves two layers:

1. **Dialect machines** consume dialect-local effects such as memory barriers, state writes,
   or analysis worklist events.
2. **The interpreter shell** is also a machine. Its effect type is `Effect<V, DE>`, and it
   owns cursor movement, SSA binding, block jumps, returns, and completion.

For the interpreter shell, `Machine(DE)` is the handoff point to the dialect machine.

## Composition Rule

Composition is uniform:

- Machine state composes by product.
- Machine effects compose by sum.
- Machine errors compose by sum.

```rust
struct MachineC {
    a: MachineA,
    b: MachineB,
}

enum EffectC {
    A(EffectA),
    B(EffectB),
}

enum ErrorC {
    A(ErrorA),
    B(ErrorB),
}

impl Machine for MachineC {
    type Effect = EffectC;
    type Error = ErrorC;

    fn consume_effect(&mut self, effect: EffectC) -> Result<(), ErrorC> {
        match effect {
            EffectC::A(effect) => self.a.consume_effect(effect).map_err(Lift::lift),
            EffectC::B(effect) => self.b.consume_effect(effect).map_err(Lift::lift),
        }
    }
}
```

## Rejected Alternative: Direct Mutation

`kirin-interpreter-3` intentionally does not expose a public `ProjectMut` path for dialect
semantics.

Why this is rejected:

1. It creates two competing semantics channels: direct mutation and effect emission.
2. It weakens ordering guarantees relative to `Seq`.
3. It makes abstract or replaying interpreters harder because some state changes are invisible
   to the effect algebra.
4. It complicates composition because direct mutation does not participate in `Lift`.

If a state change matters semantically, it belongs in `Machine(DE)`.
