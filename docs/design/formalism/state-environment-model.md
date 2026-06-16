# Part II - State & Environment Model

> Part of the [Rust Interpreter Formalism](index.md).

This part uses both shorthand (`σ`, `ρ`) and direct API names (`EnvIndex`,
`EnvStackStore`, `Ctx`) to keep proofs and implementation traces aligned.

## Reading Recipe

- **Formal read:** Read this as the state transformer substrate for `⟨s, ρ, σ⟩ ⇓_ι ...`, with `σ` and `ρ` defining where values live and how they evolve.
- **API read:** Inspect `crates/kirin-interpreter/src/{ctx.rs,env.rs}` first, then concrete/abstract `Interp` impls in `crates/kirin-interpreter/src/{concrete.rs,abstract_interp.rs}` for `env_read/env_write` behavior.

## II.0 Symbol-to-code mapping

| Formal symbol / concept | Rust type / function | Code |
| --- | --- | --- |
| Interpreter interface | `Interp` | [`crates/kirin-interpreter/src/ctx.rs`](../../../crates/kirin-interpreter/src/ctx.rs) |
| Statement runtime context | `Ctx<'_, I>` | [`crates/kirin-interpreter/src/ctx.rs`](../../../crates/kirin-interpreter/src/ctx.rs) |
| Scope env view | `EnvOps<V, E>` | [`crates/kirin-interpreter/src/ctx.rs`](../../../crates/kirin-interpreter/src/ctx.rs) |
| Environment capability | `EnvIndex` | [`crates/kirin-interpreter/src/env.rs`](../../../crates/kirin-interpreter/src/env.rs) |
| Environment trait | `Env<V>` | [`crates/kirin-interpreter/src/env.rs`](../../../crates/kirin-interpreter/src/env.rs) |
| Concrete store | `EnvStackStore<V>` | [`crates/kirin-interpreter/src/env.rs`](../../../crates/kirin-interpreter/src/env.rs) |
| Concrete `env_read` semantics | `ConcreteInterpreter` impl of `Interp` | [`crates/kirin-interpreter/src/concrete.rs`](../../../crates/kirin-interpreter/src/concrete.rs) |
| Abstract `env_read` semantics | `AbstractInterpreter` impl of `Interp` | [`crates/kirin-interpreter/src/abstract_interp.rs`](../../../crates/kirin-interpreter/src/abstract_interp.rs) |
| Structured scope carrier | `Scope<V, E>`, `ScopeBody`, `ScopeHook`, `ScopeStep` | [`crates/kirin-interpreter/src/effect.rs`](../../../crates/kirin-interpreter/src/effect.rs) |
| Value tuple packet | `Product<T>` | [`crates/kirin-ir/src/product.rs`](../../../crates/kirin-ir/src/product.rs) |

## II.1 Runtime Interfaces

The engine-facing runtime interface is `Interp`:

```rust
pub trait Interp: Sized {
    type Value: Clone;
    type Error: From<InterpreterError>;

    fn env_read(&self, env: EnvIndex, value: SSAValue) -> Result<Self::Value, Self::Error>;
    fn env_write(
        &mut self,
        env: EnvIndex,
        value: SSAValue,
        data: Self::Value,
    ) -> Result<(), Self::Error>;
}
```

Dialect code never manipulates store internals directly. It sees `Ctx<'_, I>`,
which bundles `(interp, stage, statement, env)` and provides:

- `ctx.read(x)`
- `ctx.read_many(xs)`
- `ctx.write(x, v)`
- `ctx.write_results(results, product)`

So the formal transition `σ -> σ'` for a statement is realized operationally by
mutations performed through `Ctx` over `Interp::env_write`; it is not a separate
explicit return value from `interpret`.

API-level correspondence:

- `ρ` corresponds to `Ctx::env()` / `EnvIndex`
- `σ` corresponds to engine-owned `EnvStackStore<V>`
- `σ[ρ, x] = v` corresponds to `ctx.write(x, v)` or `env_write(ρ, x, v)`

## II.2 Environment Store

Concrete storage uses `EnvStackStore<V>`:

- `stores: Vec<Option<HashMap<SSAValue, V>>>`
- `EnvIndex` is a capability (index into `stores`)
- `alloc` adds a new live record
- `free` retires a record (`None`)
- `read`/`write` are per-record SSA accesses

Formal view:

- `σ : EnvIndex -> (SSAValue -> V)` over live indices
- `alloc(σ) = (ρ, σ[ρ <- empty])`
- `free(σ, ρ)` removes liveness for `ρ`
- `write(σ, ρ, x, v)` updates `x` in record `ρ`
- `read(σ, ρ, x)` fetches bound value (or error if unbound/invalid in concrete)

## II.3 Concrete vs Abstract Read Semantics

Both engines use the same store shape, but `env_read` differs:

- **ConcreteInterpreter**: unbound read is an error.
- **AbstractInterpreter**: unbound read is `V::bottom()`.

This one rule is fundamental: abstract interpretation treats missing bindings as
"unreached/no information yet" rather than failure.

## II.4 Activation Discipline

For function calls:

1. resolve callee via linker
2. allocate callee environment
3. enter function scope with argument `Product<V>`
4. execute until return completion
5. land returned values into caller result slots
6. free callee environment

This is performed by engine/frame protocol, not by dialect statements.

## II.5 Structured Scope State

Structured control (`scf.if`, `scf.for`) is represented by `Scope<V, E>`:

- `body: ScopeBody` (`Block`, `Region`, or `Immediate`)
- `args: Product<V>` (entry arguments)
- `results: Product<SSAValue>` (landing slots)
- `hook: Option<Box<dyn ScopeHook<V, E>>>`

`ScopeHook::on_yield` receives:

- current joined `entry` state
- yielded values
- restricted env ops (`EnvOps`)

and returns `ScopeStep::{Finish, Repeat, RepeatOrFinish}`.

## II.6 Safety Obligations

1. `EnvIndex` must refer to live records on read/write.
2. Product arity must match destination slot arity.
3. Caller/callee result arity must match on return landing.
4. Scope result bindings must match yielded/finished product widths.
5. In abstract mode, joins/widening must be monotone at merge points.
