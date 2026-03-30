# Unified Effect Type

Effects are the only public language for semantically visible interpreter state changes.
Dialects describe what should happen by returning `Effect<V, DE>`, and the interpreter shell
decides how to realize that change.

## Effect Definition

```rust
enum Effect<V, DE> {
    // Cursor control
    Advance,
    Stay,
    Jump(Block, SmallVec<[V; 2]>),

    // Value binding
    BindValue(SSAValue, V),
    BindProduct(Product<ResultValue>, V),

    // Completion
    Return(V),
    Yield(V),
    Stop(V),

    // Ordered composition
    Seq(SmallVec<[Self; 2]>),

    // Dialect machine effects
    Machine(DE),
}
```

## Type Parameters

- `V` — runtime value type
- `DE` — dialect-machine effect type

Dialects with no machine effects use `Infallible`, not `()`. That keeps the impossible
`Machine(_)` case uninhabited and avoids placeholder `Lift<()>` impls.

## Combinators

```rust
impl<V, DE> Effect<V, DE> {
    fn then(self, next: Self) -> Self {
        Self::Seq(smallvec![self, next])
    }
}
```

`Seq` gives ordered composition across all observable effect kinds:

```rust
Effect::BindValue(result, v).then(Effect::Advance)

Effect::Seq(smallvec![
    Effect::BindValue(result, v),
    Effect::Machine(MemoryEffect::Flush),
    Effect::Advance,
])
```

## Composition via Lift

When a sub-dialect is embedded in a larger language, only the machine-effect payload changes:

```rust
impl<V, DEA, DEC> Lift<Effect<V, DEA>> for Effect<V, DEC>
where
    DEC: Lift<DEA>,
{
    fn lift(from: Effect<V, DEA>) -> Self {
        match from {
            Effect::Advance => Effect::Advance,
            Effect::Stay => Effect::Stay,
            Effect::Jump(block, args) => Effect::Jump(block, args),
            Effect::BindValue(ssa, value) => Effect::BindValue(ssa, value),
            Effect::BindProduct(results, value) => Effect::BindProduct(results, value),
            Effect::Return(value) => Effect::Return(value),
            Effect::Yield(value) => Effect::Yield(value),
            Effect::Stop(value) => Effect::Stop(value),
            Effect::Seq(effects) => Effect::Seq(effects.into_iter().map(Lift::lift).collect()),
            Effect::Machine(effect) => Effect::Machine(Lift::lift(effect)),
        }
    }
}
```

Composed dialect code stays mechanical:

```rust
match self {
    DialectC::A(inner) => Ok(Lift::lift(inner.interpret(interp)?)),
    DialectC::B(inner) => Ok(Lift::lift(inner.interpret(interp)?)),
}
```

## Terminal vs Consumed Effects

Effects appear in two places:

- **Consumed effects** are handled by `Interpreter::consume_effect`.
- **Terminal effects** are returned by seeds to their caller for inspection.

Typical examples:

- `BlockSeed::execute` returns a terminal `Yield`, `Return`, or `Jump`.
- An `scf.if` interpret impl can call `BlockSeed`, pattern-match on `Yield(v)`, and return
  `BindProduct(...).then(Advance)`.
- `SingleStage::consume_effect` handles `Advance`, `Jump`, `BindValue`, `Machine(de)`, and so on.

## Invariants

1. If a state change must be observed, ordered, or lifted, it must be an effect.
2. Dialects may not bypass `Effect::Machine(de)` by mutating machine state directly.
3. Seeds are not effect variants. There is no `Execute(Seed)` case in the algebra.
4. `Seq` preserves source order exactly.
