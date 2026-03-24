# State And Effects

## Dialect-Defined Machines

Each dialect defines its own semantic machine type.

The framework does not prescribe:

- one universal call-frame type
- one universal graph traversal state
- one universal loop stack

Those are language-level concerns and belong to the dialects that need them.

Examples:

- a function dialect may define call-frame machine state
- an `scf`-style dialect may define loop-resume machine state
- a graph dialect may define traversal agenda machine state

## Machine Composition

Users may compose dialect machines however they want:

- tuples
- named structs
- nested composite structs

The framework should provide typed projection traits:

- `ProjectMachine<T>`
- `ProjectMachineMut<T>`

These should be supported on:

- machine types directly
- interpreter shells as forwarding convenience

This keeps dialect bounds explicit while allowing users to define composite
machine state in ordinary Rust.

## Root Machine Access

The interpreter shell should expose:

- `machine(&self) -> &Self::Machine`
- `machine_mut(&mut self) -> &mut Self::Machine`

This is important for dialect-local tests. A dialect author should be able to
instantiate a concrete interpreter with just the machine needed for that
dialect, seed SSA values manually, and test the operational semantics without
building a whole executable language.

## Language-Owned Effects

Effects are owned by semantic machines, not by the framework.

For wrapped dialect composition:

```rust
#[wraps]
enum DialectC {
    A(DialectA),
    B(DialectB),
}
```

the composite effect should be a sum effect:

```rust
enum EffectC {
    A(EffectA),
    B(EffectB),
}
```

stop composition follows the same structural pattern:

```rust
enum StopC {
    A(StopA),
    B(StopB),
}
```

`#[wraps]` should imply effect lifting in the same way it implies semantic
delegation. Composite machines should likewise provide explicit stop lifting.

## Effect Consumption

Effect consumption is owned by machine types, not effect types:

```rust
trait ConsumeEffect<'ir>: Machine<'ir> {
    type Error;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Control<Self::Stop>, Self::Error>;
}
```

Machine effect consumption:

- mutates machine-owned semantic state
- returns shell-facing `Control<Self::Stop>`

This is the semantic-to-shell boundary.

Interpreter shells should expose both local and lifted consumption helpers:

- `consume_local_effect(effect)`
- `consume_lifted_effect(effect)`
- `consume_effect(effect)`
- `consume_local_control(control)`
- `consume_control(control)`

`consume_local_effect` mutates only the projected submachine.
`consume_control` mutates only the interpreter shell.

## Value Store Placement

The framework should not require dynamic interpreters to expose one universal
typed `ValueStore`.

Instead:

- typed single-stage interpreters expose `ValueStore`
- typed stage handles expose `ValueStore`
- the dynamic shell itself does not expose raw typed value APIs

For typed execution, `ValueStore` should provide:

- `read` and `write` generic over `Into<SSAValue>`
- `read_many`
- `write_many`

`read_many` and `write_many` should use raw `Product<V>` directly and should
not include a framework-owned `write_product` policy.

## Result Conventions

Result packing and unpacking remain language-defined.

The framework does not define semantic effects like `Return` or `Yield`.
If a dialect wants those, it defines:

- the effect variants
- the machine state it needs
- the `ConsumeEffect` behavior on that machine

This applies equally to:

- call/return conventions
- yield conventions
- graph output conventions
- multi-result sugar over tuple/product values

## Local And Lifted APIs

The shell should support both local and top-level views of semantics.

Interpret:

- `interpret_local(stmt)` returns `Sub::Effect`
- `interpret_lifted(stmt)` returns `I::Machine::Effect`
- `interpret_current()` returns `I::Machine::Effect`

Consume:

- `consume_local_effect(effect)` returns `Control<Sub::Stop>`
- `consume_lifted_effect(effect)` returns `Control<I::Machine::Stop>`
- `consume_effect(effect)` returns `Control<I::Machine::Stop>`

This split is the core testing story:

- dialect-unit tests use local interpret/consume APIs
- full-language stepping uses lifted/top-level APIs
- the same interpreter shell supports both
