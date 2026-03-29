# Layered Effect Types

Effects are the language through which dialects and interpreters communicate. The effect system is layered:
a base set shared by all interpreters, interpreter-specific extensions parameterized by the dialect's
machine effect type.

## Base Effects

The minimum set of effects that all interpreters support. Parameterized only by value type `V`:

```rust
enum BaseEffect<V> {
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
}
```

`Seq` is a first-class combinator for expressing multiple effects in order:

```rust
impl<V> BaseEffect<V> {
    fn then(self, next: Self) -> Self {
        Self::Seq(smallvec![self, next])
    }
}
```

## Interpreter-Specific Effects (GAT)

Each interpreter type defines its effect as a **GAT** parameterized by the dialect's machine effect:

```rust
// SingleStage: base + seeds + machine effects
enum SingleStageEffect<V, Seed, DE> {
    Base(BaseEffect<V>),
    Execute(Seed),
    Machine(DE),
}

// Abstract: base + seeds + fork + machine effects
enum AbstractEffect<V, Seed, DE> {
    Base(BaseEffect<V>),
    Execute(Seed),
    Fork(Vec<(Block, SmallVec<[V; 2]>)>),
    Machine(DE),
}
```

The GAT on the `Interpreter` trait:

```rust
trait Interpreter: Machine + ValueRead + PipelineAccess {
    type Effect<DE>: TryLift<BaseEffect<<Self as ValueRead>::Value>>
                   + TryLift<DE>;
    // ...
}
```

## Lifting Into Interpreter Effects

Dialects construct the interpreter's effect type via `try_lift()`:

- **Base effects** (Advance, BindValue, etc.) → `try_lift()` maps to the `Base(...)` slot
- **Machine effects** (dialect-specific) → `try_lift()` maps to the `Machine(...)` slot
- **Execute, Fork** — interpreter-internal, never produced by dialects

The framework provides Lift impls on each interpreter's effect type:

```rust
// BaseEffect always lifts into interpreter effects
impl<V, Seed, DE> Lift<BaseEffect<V>> for SingleStageEffect<V, Seed, DE> {
    fn lift(from: BaseEffect<V>) -> Self { Self::Base(from) }
}

// Machine effects always lift into interpreter effects
impl<V, Seed, DE> Lift<DE> for SingleStageEffect<V, Seed, DE> {
    fn lift(from: DE) -> Self { Self::Machine(from) }
}
```

Since both go through `TryLift` (blanket from `Lift`), dialects use a single uniform API: `.try_lift()`.

## Composition: Lift Between GAT Instantiations

When composing sub-dialect effects, the framework provides a Lift between different instantiations
of the same interpreter effect GAT. If `DE2: Lift<DE>`, then `I::Effect<DE2>: Lift<I::Effect<DE>>`:

```rust
impl<V, Seed, DE, DE2> Lift<SingleStageEffect<V, Seed, DE>> for SingleStageEffect<V, Seed, DE2>
where DE2: Lift<DE>
{
    fn lift(from: SingleStageEffect<V, Seed, DE>) -> Self {
        match from {
            SingleStageEffect::Base(base) => Self::Base(base),
            SingleStageEffect::Execute(seed) => Self::Execute(seed),
            SingleStageEffect::Machine(de) => Self::Machine(Lift::lift(de)),
        }
    }
}
```

Composed dialects use the same `try_lift()` as everywhere else:

```rust
match self {
    DialectC::A(inner) => inner.interpret(interp)?.try_lift(),
    DialectC::B(inner) => inner.interpret(interp)?.try_lift(),
}
```

## Symmetry with Errors

The effect and error models are intentionally symmetric. See [errors.md](errors.md).

## Builder Patterns (Deferred)

Ergonomic builders for common effect patterns will be added during implementation.
