# Part II - State & Environment Model

> Part of the [Rust Interpreter Formalism](index.md).

This part uses both shorthand (`σ`, `ρ`) and direct API names (`EnvIndex`,
`EnvStore`, `Env`, `Interp`) to keep proofs and implementation traces
aligned.

## Reading Recipe

- **Formal read:** Read this as the state transformer substrate for `⟨s, ρ, σ⟩ ⇓_ι ...`, with `σ` and `ρ` defining where values live and how they evolve.
- **API read:** Inspect `crates/kirin-interpreter/src/core/{interp.rs,env/}` first, then the concrete/forward-abstract `Env` impls in `crates/kirin-interpreter/src/engines/{concrete,sparse_forward}/interp.rs` for `env_read/env_write` behavior.

## II.0 Symbol-to-code mapping

| Formal symbol / concept | Rust type / function | Code |
| --- | --- | --- |
| Interpreter interface | `Interp` | [`core/interp.rs`](../../../crates/kirin-interpreter/src/core/interp.rs) |
| Statement location | `InterpLocation` | [`core/interp.rs`](../../../crates/kirin-interpreter/src/core/interp.rs) |
| Forward eval helpers | `SparseForwardInterp` | [`core/interp.rs`](../../../crates/kirin-interpreter/src/core/interp.rs) |
| Environment capability | `EnvIndex` | [`core/env/store.rs`](../../../crates/kirin-interpreter/src/core/env/store.rs) |
| Environment access trait | `Env` | [`core/env/services.rs`](../../../crates/kirin-interpreter/src/core/env/services.rs) |
| Activation lifetime | `CallServices` (`alloc_env`/`free_env`; a sibling of `Env`) | [`core/frame.rs`](../../../crates/kirin-interpreter/src/core/frame.rs) |
| Environment container | `EnvStore<K, A, V>` | [`core/env/store.rs`](../../../crates/kirin-interpreter/src/core/env/store.rs) |
| Shared fact map | `FactStore<A, V>` | [`facts/store.rs`](../../../crates/kirin-interpreter/src/facts/store.rs) |
| Concrete `env_read` semantics | `ConcreteInterpreterCore` impl of `Env` | [`engines/concrete/interp.rs`](../../../crates/kirin-interpreter/src/engines/concrete/interp.rs) |
| Forward abstract `env_read` semantics | `SparseForwardTransfer` impl of `Env` | [`engines/sparse_forward/interp.rs`](../../../crates/kirin-interpreter/src/engines/sparse_forward/interp.rs) |
| Structured scope carrier | `Scope<V, E>`, `ScopeBody`, `ScopeHook`, `ScopeStep` | [`crates/kirin-interpreter/src/effect.rs`](../../../crates/kirin-interpreter/src/effect.rs) |
| Value tuple packet | `Product<T>` | [`crates/kirin-ir/src/product.rs`](../../../crates/kirin-ir/src/product.rs) |

## II.1 Runtime Interfaces

The engine-facing runtime interface is `Interp`:

```rust
pub trait Interp: Sized {
    type Value: Clone;
    type Error: From<InterpreterError>;
    type Effect;
    type Kind;

    fn stage(&self) -> CompileStage;
    fn statement(&self) -> Statement;
    fn index(&self) -> EnvIndex;
}
```

Forward-evaluation dialect code never manipulates store internals directly. It
receives `&mut I`, where `I: SparseForwardInterp`; the engine has already stashed
the current `(stage, statement, env)` as an `InterpLocation` and exposes:

- `interp.read(x)`
- `interp.read_many(xs)`
- `interp.write(x, v)`
- `interp.write_results(results, product)`

So the formal transition `σ -> σ'` for a statement is realized operationally by
mutations performed through `SparseForwardInterp` helpers over
`Env::env_write`; it is not a separate explicit return value from
`interpret`.

API-level correspondence:

- `ρ` corresponds to `interp.index()` / `EnvIndex`
- `σ` corresponds to the engine-owned `EnvStore<K, SSAValue, V>` container
- `σ[ρ, x] = v` corresponds to `interp.write(x, v)` or `env_write(ρ, x, v)`

## II.2 Environment Container

Concrete and forward abstract storage use `EnvStore<K, A, V>`:

- `context_indices: HashMap<K, EnvIndex>` — which analysis context an environment belongs to
- `environments: Vec<Option<Environment<K, A, V>>>` — each holding one `FactStore<A, V>`
- `EnvIndex` is a capability (index into `environments`), never reused after `free`
- `alloc` adds a new live record with no context association
- `get_or_allocate(k)` returns `k`'s live record, adding one on first use
- `free` retires a record and drops its context association
- `read`/`write` are per-record anchor accesses; `environment` inspects a record's fact map

The container maps context identity to storage and nothing else: the *analysis*
chooses `K` (see `CallContext`), `write` assigns rather than joins, and `read`
reports an absent anchor as absent, leaving absence semantics, context selection,
and convergence to the engine above it. Concrete execution instantiates
`K = Infallible`, so it can only `alloc`. Backward analyses use `FactStore`
directly with their existing scoped anchors.

Formal view:

- `σ : EnvIndex -> (A -> V)` over live indices, plus `δ : K -> EnvIndex` over live contexts
- `alloc(σ) = (ρ, σ[ρ <- empty])`
- `getOrAlloc(σ, δ, k) = (δ(k), σ, δ)` if `k ∈ dom δ`, else `(ρ, σ[ρ <- empty], δ[k <- ρ])`
- `free(σ, δ, ρ)` removes liveness for `ρ` and drops `k` with `δ(k) = ρ`
- `write(σ, ρ, x, v)` updates `x` in record `ρ`
- `read(σ, ρ, x)` fetches the stored value, or reports absence (the engine decides: error in concrete, `⊥` in abstract)

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

- `body: ScopeBody` (`Block`, `CFG`, or `Immediate`)
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
