# State And Effects

## Dialect-Defined State

Each dialect defines its own semantic state type.

The framework does not prescribe:

- one universal call-frame type
- one universal graph traversal state
- one universal loop stack

Those are language-level concerns and belong to the dialects that need them.

Examples:

- a function dialect may define call-frame state
- an `scf`-style dialect may define loop-resume state
- a graph dialect may define traversal agenda state

## State Composition

Users may compose dialect state however they want:

- tuples
- named structs
- nested composite structs

The framework should provide typed projection traits:

- `ProjectState<T>`
- `ProjectStateMut<T>`

These should be supported on:

- state payload types directly
- interpreter shells as forwarding convenience

This keeps dialect bounds explicit while allowing users to define composite
state in ordinary Rust.

## Root State Access

The machine shell should expose:

- `state(&self) -> &Self::State`
- `state_mut(&mut self) -> &mut Self::State`

This is important for dialect-local tests. A dialect author should be able to
instantiate a concrete interpreter with just the state needed for that dialect,
seed SSA values manually, and test the operational semantics without building a
whole executable language.

## Language-Owned Effects

Effects are owned by the language semantics, not by the framework.

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

`#[wraps]` should imply effect lifting in the same way it implies semantic
delegation.

## Effect Consumption

Each language effect implements `ConsumeEffect<'ir, I>`.

Effect consumption:

- mutates dialect-owned semantic state
- may use public interpreter helpers
- returns a framework-defined `MachineAction<I::Stop>`

This is the semantic-to-machine boundary.

The framework should expose both:

- two-phase APIs
  - `consume_effect(effect) -> MachineAction<Stop>`
  - `apply_action(action)`
- convenience API
  - `consume_and_apply(effect)`

This is useful for tests and custom drivers.

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
- the state it needs
- the `ConsumeEffect` behavior

This applies equally to:

- call/return conventions
- yield conventions
- graph output conventions
- multi-result sugar over tuple/product values
