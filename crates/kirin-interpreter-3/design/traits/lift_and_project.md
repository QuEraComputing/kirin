# Lift/Project Algebra

The `Lift/Project` and `TryLift/TryProject` traits formally define the hierarchy of dialect composition.
They mirror Rust's `From/Into` and `TryFrom/TryInto` pattern — infallible and fallible conversions
with source-side convenience methods and blanket impls connecting them.

## Trait Definitions

### Infallible Conversions

```rust
/// Embed a value into a larger type (like From — defined on target)
trait Lift<From> {
    fn lift(from: From) -> Self;
}

/// Extract a value from a larger type (like Into — but for projection)
trait Project<To> {
    fn project(self) -> To;
}
```

### Fallible Conversions

```rust
/// Try to embed a value (like TryFrom — defined on target)
trait TryLift<From>: Sized {
    type Error;
    fn try_lift(from: From) -> Result<Self, Self::Error>;
}

/// Try to extract a value (like TryProject — defined on source)
trait TryProject<To>: Sized {
    type Error;
    fn try_project(self) -> Result<To, Self::Error>;
}
```

### Source-Side Convenience (like Into/TryInto)

```rust
trait LiftInto<Target>: Sized {
    fn lift_into(self) -> Target;
}

trait TryLiftInto<Target>: Sized {
    type Error;
    fn try_lift(self) -> Result<Target, Self::Error>;
}
```

### Blanket Impls

```rust
// Identity
impl<T> Lift<T> for T { fn lift(from: T) -> Self { from } }
impl<T> Project<T> for T { fn project(self) -> T { self } }

// Lift → TryLift (infallible always succeeds)
impl<F, T: Lift<F>> TryLift<F> for T {
    type Error = Infallible;
    fn try_lift(from: F) -> Result<Self, Infallible> { Ok(Self::lift(from)) }
}

// Project → TryProject (infallible always succeeds)
impl<F: Project<T>, T> TryProject<T> for F {
    type Error = Infallible;
    fn try_project(self) -> Result<T, Infallible> { Ok(self.project()) }
}

// Source-side: Lift → LiftInto
impl<F, T: Lift<F>> LiftInto<T> for F {
    fn lift_into(self) -> T { T::lift(self) }
}

// Source-side: TryLift → TryLiftInto
impl<F, T: TryLift<F>> TryLiftInto<T> for F {
    type Error = <T as TryLift<F>>::Error;
    fn try_lift(self) -> Result<T, Self::Error> { T::try_lift(self) }
}
```

## Orphan Rule Rationale

Lift/Project are defined on the **composite type** (e.g., `DialectC`), not the leaf types (e.g., `DialectA`).
This avoids orphan rule violations because the composite type is defined later in a downstream crate.

## Composition Rules

- **Machines** compose by **product** (struct of sub-machines): `MachineC { a: MachineA, b: MachineB }`
- **Effects** compose by **sum** (enum of sub-effects): `EffectC = EffectA | EffectB`
- **Errors** compose by **sum** (enum of sub-errors): `ErrorC = ErrorA | ErrorB`
- **Dialects** compose by **sum** (already how `#[derive(Dialect)]` works with `#[wraps]`)

For mutable machine projection:

```rust
trait ProjectMut<To> {
    fn project_mut(&mut self) -> &mut To;
}

impl<T> ProjectMut<T> for T {
    fn project_mut(&mut self) -> &mut T { self }
}
```

## Usage in Interpreter-3

The Lift/Project algebra is used at three levels:

1. **Effect composition**: `Lift<Effect<V, S, DEA>> for Effect<V, S, DEC>` where `DEC: Lift<DEA>`.
   Only the `Machine(de)` variant is transformed; all other effect variants pass through unchanged.
   See [effects.md](effects.md).

2. **Error composition**: `Lift<InterpError<MEA>> for InterpError<MEC>` where `MEC: Lift<MEA>`.
   Same pattern — only the `Machine(me)` variant is transformed.
   See [errors.md](errors.md).

3. **Dialect machine composition**: `Lift<EffectA> for EffectC`, `Lift<ErrorA> for ErrorC` for
   composing sub-dialect machine effects/errors into a parent enum. See [machine.md](machine.md).

`TryProject` is used by seeds to pattern-match on terminal effects (e.g., extracting
`Effect::Yield(v)` from a block's terminal effect).

## Implementation Plan

Manual `Lift/Project` definitions first. Eventually move trait definitions into `kirin-ir` and
implement `derive(Dialect)` support for generating `Lift/Project` pairs automatically.
