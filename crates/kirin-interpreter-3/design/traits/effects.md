# Unified Effect Type

Effects are the language through which dialects and interpreters communicate. A single
`Effect<V, Seed, DE>` type expresses everything — cursor control, value binding, completion,
complex execution, and dialect machine effects.

## Effect Definition

```rust
enum Effect<V, Seed, DE> {
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

    // Composition
    Seq(SmallVec<[Self; 2]>),

    // Complex execution (seeds)
    Execute(Seed),

    // Dialect machine effects
    Machine(DE),
}
```

**Type parameters:**

- `V` — runtime value type (e.g., `i64`, `Value`)
- `Seed` — seed type for complex execution (e.g., `CompositeSeed<V>`). Dialects never construct
  `Execute` directly — they create seeds and execute them via `&mut I`.
- `DE` — dialect machine effect type. `()` for pure dialects, a custom enum for stateful dialects.

Both `Interpretable::interpret` and `Machine::consume_effect` use this same type. For an
interpreter `I`, the concrete effect is `Effect<I::Value, I::Seed, I::DialectEffect>`, which
is also `I::Effect` (the `Machine::Effect` associated type).

## Combinators

```rust
impl<V, Seed, DE> Effect<V, Seed, DE> {
    fn then(self, next: Self) -> Self {
        Self::Seq(smallvec![self, next])
    }
}
```

`Seq` composes any effects — base, machine, or mixed:

```rust
Effect::BindValue(result, v).then(Effect::Advance)

Effect::Seq(smallvec![
    Effect::BindValue(result, v),
    Effect::Machine(MemoryEffect::Flush),
    Effect::Advance,
])
```

## Composition via Lift

When composing sub-dialect effects into a larger dialect, `Lift` converts between
`Effect` types with different `DE` parameters:

```rust
impl<V, Seed, DEA, DEC> Lift<Effect<V, Seed, DEA>> for Effect<V, Seed, DEC>
where DEC: Lift<DEA>
{
    fn lift(from: Effect<V, Seed, DEA>) -> Self {
        match from {
            Effect::Advance => Effect::Advance,
            Effect::Stay => Effect::Stay,
            Effect::Jump(b, a) => Effect::Jump(b, a),
            Effect::BindValue(s, v) => Effect::BindValue(s, v),
            Effect::BindProduct(p, v) => Effect::BindProduct(p, v),
            Effect::Return(v) => Effect::Return(v),
            Effect::Yield(v) => Effect::Yield(v),
            Effect::Stop(v) => Effect::Stop(v),
            Effect::Seq(effs) => Effect::Seq(effs.into_iter().map(Lift::lift).collect()),
            Effect::Execute(s) => Effect::Execute(s),
            Effect::Machine(de) => Effect::Machine(Lift::lift(de)),
        }
    }
}
```

Only the `Machine(de)` variant is transformed — all other variants pass through unchanged.

Composed dialect code:

```rust
match self {
    DialectC::A(inner) => Ok(Lift::lift(inner.interpret(interp)?)),
    DialectC::B(inner) => Ok(Lift::lift(inner.interpret(interp)?)),
}
```

## Terminal vs Consumed Effects

Effects follow two paths through the system:

- **Consumed**: The interpreter's `consume_effect` processes the effect (mutates state, advances cursor, etc.) and returns `()`.
- **Terminal**: A seed runs a block and returns the terminator's effect to its caller *without* consuming it. The caller pattern-matches to decide what to do.

For example, `BlockSeed::execute` steps through all non-terminator statements (consuming each effect), then returns the terminator's effect. `IfSeed::execute` matches on `Yield(v)` to extract the result.

## Abstract Interpretation

The `Effect` type does not include a `Fork` variant. Abstract interpreters that need
nondeterministic branching express it as a machine effect: `Effect::Machine(AnalysisEffect::Fork(...))`.
This keeps Fork out of the concrete execution path entirely.

## Symmetry with Errors

The error model follows the same pattern. See [errors.md](errors.md).
