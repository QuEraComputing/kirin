# Derive And Tooling

## Scope

Under the machine design, `kirin-derive-interpreter-2` should cover two
separate concerns:

- dialect-side statement semantics forwarding
- machine-side structural composition

Those are related, but they are not the same derive problem and should stay
explicit in the user surface.

## Dialect-Side Derives

The dialect-side derive family remains centered on `Interpretable`.

`#[wraps]` stays a dialect-side helper attribute for semantic delegation on
wrapped dialect enums and wrapper types. It should not be reused for machine
composition.

The existing `#[interpret(...)]` namespace remains the right home for
interpreter-derive configuration.

`Interpretable` remains the semantic-only downstream contract:

- the generated impl should depend on `I: Interpreter<'ir>`
- it should not require `I: interpreter::Position<'ir>`
- it should not require `I: interpreter::Driver<'ir>`
- it should not require control traits such as `control::Fuel`

Driver and position APIs are for shell authors, typed stage views, and tooling,
not for ordinary dialect semantics.

Under the machine design, `#[derive(Interpretable)]` should always require an
explicit machine binding:

```rust
#[derive(Interpretable)]
#[interpret(machine = MachineC)]
enum DialectC {
    #[wraps]
    A(DialectA),
    #[wraps]
    B(DialectB),
}
```

This should be required for:

- leaf statement types
- wrapper structs
- wrapper enums
- composite dialect enums

The derive should not try to infer the machine by naming convention.

## Wrapper `Interpretable` Behavior

For wrapper dialect types, the generated impl should return the wrapper
machine's effect type directly.

If:

```rust
#[derive(Interpretable)]
#[interpret(machine = MachineC)]
enum DialectC {
    #[wraps]
    A(DialectA),
    #[wraps]
    B(DialectB),
}
```

then the generated impl should conceptually behave like:

```rust
match self {
    Self::A(inner) => <MachineC as LiftEffect<'ir, MachineA>>::lift_effect(
        inner.interpret(interp)?,
    ),
    Self::B(inner) => <MachineC as LiftEffect<'ir, MachineB>>::lift_effect(
        inner.interpret(interp)?,
    ),
}
```

The important point is that wrapper forwarding lifts into the declared local
wrapper machine (`MachineC`), not into the interpreter shell's top-level
machine.

This is why the explicit outer machine binding is part of the derive contract.

## Wrapper Bounds

For wrapped variants, the derive should auto-generate the local wrapper-machine
lifting bounds it needs:

- `MachineC: LiftEffect<'ir, MachineA>`
- `MachineC: LiftEffect<'ir, MachineB>`

It should not require wrapped variants to restate their own machine bindings in
attributes.

It should also not redundantly restate interpreter-top-level projection bounds
for the wrapped inner dialects. The inner type's own `Interpretable<'ir, I>`
impl already carries the interpreter requirements it actually needs.

So the wrapper derive rule is:

- the outer derived type declares `#[interpret(machine = ...)]`
- wrapped inner variants do not declare machine bindings
- the derive reads the inner machine from `Inner: Interpretable<'ir, I>`
- the derive generates local wrapper-machine `LiftEffect` bounds automatically

## Machine-Side Derive Family

Machine composition should use an explicit machine-side derive family with its
own namespace:

- `#[derive(Machine)]`
- `#[derive(ProjectMachine)]`
- `#[derive(LiftEffect)]`
- `#[derive(LiftStop)]`

and a separate machine attribute namespace:

- `#[machine(...)]`

This keeps the split clear:

- `#[wraps]` is for dialect-side delegation
- `#[machine(...)]` is for structural machine composition

## `Machine` Derive

`#[derive(Machine)]` should stay thin and declarative.

It should require both top-level type annotations explicitly:

```rust
#[derive(Machine)]
#[machine(effect = EffectC, stop = StopC)]
struct MachineC {
    #[machine(sub)]
    a: MachineA,
    #[machine(sub)]
    b: MachineB,
}
```

It should generate only:

- `impl Machine<'ir> for MachineC`

It should not try to validate cross-derive structural properties. Those belong
to the structural derives.

This family should support:

- named-field structs
- tuple structs

It should not support enums, because machine composition is product-like rather
than sum-like.

## `ProjectMachine` Derive

`#[derive(ProjectMachine)]` should generate both:

- `ProjectMachine<T>`
- `ProjectMachineMut<T>`

Selection should be explicit:

- fields participating in machine composition must be marked `#[machine(sub)]`

Example:

```rust
#[derive(Machine, ProjectMachine)]
#[machine(effect = EffectC, stop = StopC)]
struct MachineC {
    #[machine(sub)]
    a: MachineA,
    #[machine(sub)]
    b: MachineB,
}
```

This should expand to structural projection impls for `MachineA` and `MachineB`
on `MachineC`.

The derive should:

- require at least one `#[machine(sub)]` field
- reject duplicate submachine types among `#[machine(sub)]` fields
- support both named-field and tuple structs

It should catch obvious structural misuse early, but it should not attempt full
trait-resolution logic that Rust itself should diagnose.

## `LiftEffect` And `LiftStop`

`#[derive(LiftEffect)]` and `#[derive(LiftStop)]` should stay separate derives.

They should use the same `#[machine(sub)]` field selection by default.

The key design choice is that lifting should rely on ordinary Rust conversions,
not derive-specific constructor-path attributes.

For a composite machine:

```rust
struct MachineC {
    a: MachineA,
    b: MachineB,
}

enum EffectC {
    A(EffectA),
    B(EffectB),
}

enum StopC {
    A(StopA),
    B(StopB),
}
```

the derives should require ordinary conversions such as:

```rust
impl From<EffectA> for EffectC {
    fn from(value: EffectA) -> Self {
        EffectC::A(value)
    }
}

impl From<EffectB> for EffectC {
    fn from(value: EffectB) -> Self {
        EffectC::B(value)
    }
}

impl From<StopA> for StopC {
    fn from(value: StopA) -> Self {
        StopC::A(value)
    }
}

impl From<StopB> for StopC {
    fn from(value: StopB) -> Self {
        StopC::B(value)
    }
}
```

Then:

- `#[derive(LiftEffect)]` can generate `LiftEffect<'ir, MachineA>` and
  `LiftEffect<'ir, MachineB>` by calling `From::from`
- `#[derive(LiftStop)]` can generate `LiftStop<'ir, MachineA>` and
  `LiftStop<'ir, MachineB>` by calling `From::from`

This is more explicit and more Rust-native than field annotations such as
`#[machine(effect = EffectC::A)]`.

## User Surface Examples

### Minimal Single-Dialect Machine

```rust
#[derive(Machine)]
#[machine(effect = FunctionEffect, stop = FunctionStop)]
struct FunctionMachine {
    frames: Vec<CallFrame>,
}
```

This only declares the machine trait. There is no structural composition
derive because there are no submachines.

### Composite Machine

```rust
#[derive(Machine, ProjectMachine, LiftEffect, LiftStop)]
#[machine(effect = EffectC, stop = StopC)]
struct MachineC {
    #[machine(sub)]
    a: MachineA,
    #[machine(sub)]
    b: MachineB,
}
```

This machine:

- declares `EffectC` and `StopC` as its top-level semantic payloads
- projects to `MachineA` and `MachineB`
- lifts `EffectA` / `EffectB` into `EffectC`
- lifts `StopA` / `StopB` into `StopC`

The user still writes the ordinary `From` impls explicitly.

### Tuple-Struct Composition

```rust
#[derive(Machine, ProjectMachine, LiftEffect, LiftStop)]
#[machine(effect = EffectC, stop = StopC)]
struct MachineC(
    #[machine(sub)] MachineA,
    #[machine(sub)] MachineB,
);
```

Tuple structs should be supported the same way as named-field structs.

### Dialect And Machine Stay Separate

```rust
#[derive(Interpretable)]
#[interpret(machine = MachineC)]
enum DialectC {
    #[wraps]
    A(DialectA),
    #[wraps]
    B(DialectB),
}

#[derive(Machine, ProjectMachine, LiftEffect, LiftStop)]
#[machine(effect = EffectC, stop = StopC)]
struct MachineC {
    #[machine(sub)]
    a: MachineA,
    #[machine(sub)]
    b: MachineB,
}
```

The important point is that the dialect derive and machine derives are explicit
and separate. There is no cross-type inference from the wrapped dialect enum
into the machine struct.

### Wrapper Effect Lifting Is Local

```rust
#[derive(Interpretable)]
#[interpret(machine = MachineC)]
enum DialectC {
    #[wraps]
    A(DialectA),
    #[wraps]
    B(DialectB),
}
```

The generated wrapper impl should return `EffectC`, not `EffectA` or `EffectB`.

It should do that by lifting into `MachineC` locally, not by asking the
interpreter shell to lift into whatever its top-level machine happens to be.

That keeps wrapper composition reusable inside larger top-level machines.

## Validation Rules

The machine-side derive family should enforce these structural rules:

- `Machine` requires explicit `effect` and `stop` type paths
- `ProjectMachine` requires at least one `#[machine(sub)]` field
- duplicate `#[machine(sub)]` field types are rejected
- only structs are supported
- named-field and tuple-struct forms are supported

The derives should not try to eagerly prove Rust trait obligations such as:

- `From<Sub::Effect> for Self::Effect`
- `From<Sub::Stop> for Self::Stop`

Those should be emitted in generated impls and left for Rust to diagnose if
missing.

## Toolkit Implications

`kirin-derive-toolkit` should grow reusable machine-composition helpers for:

- collecting `#[machine(sub)]` fields
- generating immutable and mutable projection impls
- generating lifting impls that call `From::from`
- producing clear duplicate-submachine diagnostics

This should remain separate from the older wrapper-forwarding templates used
for dialect-side `Interpretable` delegation, even if some low-level attribute
and layout utilities can be shared.
