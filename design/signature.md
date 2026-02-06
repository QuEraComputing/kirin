# Signature and Specialization Design

## Motivation

Kirin models functions as **abstract callable objects** that may be specialized into different representations across compilation stages. A `Function` can have multiple `StagedFunction`s (one per compilation stage), each of which can have multiple `SpecializedFunction`s (one per input signature variant).

The previous design coupled signatures directly to `TypeLattice`, requiring every type system to implement lattice operations (`join`, `meet`, `is_subseteq`, `top`, `bottom`). This is too restrictive:

- Simple nominal type systems (e.g., `i32`, `f64`) don't need lattice structure.
- Advanced type systems with constraints (resource types, dependent types, refinement types) need richer comparison logic that doesn't fit the lattice model.

The new design **decouples signatures from lattice operations** and makes specialization behavior fully customizable.

## Architecture

```
Signature<T, C>          -- generic signature, T = type, C = constraints
    |
SignatureSemantics<T, C>  -- trait: how to match & compare signatures
    |
    ├── ExactSemantics     -- builtin: exact equality matching
    ├── LatticeSemantics   -- builtin: subtype-based matching via TypeLattice
    └── (user-defined)     -- custom: constraint solving, dependent types, etc.
```

### `Signature<T, C = ()>`

A function signature parameterized over the type `T` and optional constraints `C`:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Signature<T, C = ()> {
    pub params: Vec<T>,
    pub ret: T,
    pub constraints: C, // defaults to () for simple languages
}
```

- `params` — the parameter types.
- `ret` — the return type.
- `constraints` — an extensible slot for constraint contexts (type-variable bindings, refinements, resource bounds, etc.). Defaults to `()` so simple languages pay no cost.

### `SignatureSemantics<T, C>`

The trait that defines how signatures are matched and compared during specialization dispatch:

```rust
pub trait SignatureSemantics<T, C = ()> {
    type Env; // produced when a candidate is found applicable

    fn applicable(call: &Signature<T, C>, cand: &Signature<T, C>) -> Option<Self::Env>;

    fn cmp_candidate(
        a: &Signature<T, C>, a_env: &Self::Env,
        b: &Signature<T, C>, b_env: &Self::Env,
    ) -> SignatureCmp;
}
```

- **`Env`** — an associated type for environment data produced during applicability checking (e.g., solved type-variable bindings, constraint solutions). For simple semantics this is `()`.
- **`applicable`** — determines whether a candidate specialization can handle a given call signature. Returns `Some(env)` with the matching environment if applicable.
- **`cmp_candidate`** — given two applicable candidates, determines which is more specific. Used to select the best match when multiple specializations apply.

### `SignatureCmp`

```rust
pub enum SignatureCmp {
    More,         // left is more specific
    Less,         // left is less specific
    Equal,
    Incomparable, // neither is more specific
}
```

## Builtin Semantics

### `ExactSemantics`

Applicable only when params, ret, and constraints are exactly equal. `cmp_candidate` returns `Equal` when signatures match, `Incomparable` otherwise. No lattice required.

Use case: simple nominal type systems where overloading is by exact type match.

### `LatticeSemantics<T: TypeLattice>`

Applicable when all call params are subtypes (`is_subseteq`) of the candidate params. `cmp_candidate` compares specificity pairwise.

Use case: languages with subtyping where `f(Int)` should match a call with `f(PositiveInt)` because `PositiveInt <: Int`.

## Custom Semantics — Constraint-Based Specialization

The `constraints: C` field and `SignatureSemantics` trait together enable rich specialization for advanced type systems. Here are examples of what becomes possible.

### Example: Resource / Duration Constraints

Consider a language with resource-typed parameters where functions carry constraints on type variables:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DurationConstraint {
    type_var: String,     // e.g., "Duration"
    upper_bound: u64,     // e.g., 4, 6
}

type ResourceSignature = Signature<ResourceType, Vec<DurationConstraint>>;
```

A staged function might declare the general signature:

```
fn process(stream: Stream<Duration>) where Duration < 10
```

Two specializations could exist:

```
fn process(stream: Stream<Duration>) where Duration < 4   // optimized fast path
fn process(stream: Stream<Duration>) where Duration < 6   // medium path
```

A custom `ResourceSemantics` implementation would:

1. In `applicable`: check whether the call's duration constraint is satisfiable within the candidate's constraint bounds. A call with `Duration < 3` is applicable to both candidates.
2. In `cmp_candidate`: the candidate with the tighter bound (`Duration < 4`) is more specific than `Duration < 6`, so it returns `More`.

```rust
struct ResourceSemantics;

impl SignatureSemantics<ResourceType, Vec<DurationConstraint>> for ResourceSemantics {
    type Env = ConstraintSolution; // solved bindings

    fn applicable(
        call: &Signature<ResourceType, Vec<DurationConstraint>>,
        cand: &Signature<ResourceType, Vec<DurationConstraint>>,
    ) -> Option<Self::Env> {
        // Check each call constraint is satisfiable under candidate constraints
        solve_constraints(&call.constraints, &cand.constraints)
    }

    fn cmp_candidate(
        a: &Signature<ResourceType, Vec<DurationConstraint>>,
        _a_env: &Self::Env,
        b: &Signature<ResourceType, Vec<DurationConstraint>>,
        _b_env: &Self::Env,
    ) -> SignatureCmp {
        // Tighter constraints = more specific
        compare_constraint_tightness(&a.constraints, &b.constraints)
    }
}
```

### Example: Dependent Types

For a dependently-typed language where types carry value-level information:

```rust
type DepSignature = Signature<DepType, TypeVarEnv>;
```

The `Env` associated type could carry unification results, and `applicable` would perform type-level constraint solving.

### Example: Effect Systems

```rust
type EffectSignature = Signature<BaseType, EffectSet>;
```

Where `EffectSet` tracks which side effects a function may perform, and specialization prefers candidates with fewer effects.

## Dialect Trait Change

The `Dialect` associated type was loosened:

```rust
// Before
pub trait Dialect {
    type TypeLattice: TypeLattice;  // required lattice ops
}

// After
pub trait Dialect {
    type Type: CompileTimeValue + Default;  // minimal bound
}
```

- **`CompileTimeValue`** = `Clone + Debug + Hash + PartialEq` — the minimum for IR storage.
- **`Default`** — provides placeholder values (replaces `FiniteLattice::top()`).
- **`TypeLattice`** still exists and now additionally requires `Default`. Languages that need lattice operations can still use `T: TypeLattice` bounds and `LatticeSemantics`.

The derive attribute changed accordingly: `#[kirin(type_lattice = T)]` became `#[kirin(type = T)]`.

## Pipeline Alignment

When specifying a compilation pipeline (e.g., language A -> B -> C), all languages should use the **same `SignatureSemantics`** so that signatures are aligned across stages. Even though the underlying type `T` differs between stages, the semantics of "what does it mean for one signature to be more specific than another" must be consistent.

## Integration with Function Model

```
Function                     -- abstract callable, has a name
  └── StagedFunction         -- one per compilation stage
        ├── signature: Signature<L::Type>    -- user-declared signature
        └── SpecializedFunction              -- one per specialization
              ├── signature: Signature<L::Type>  -- compiler-derived signature
              └── body: Statement                -- structural container
```

`StagedFunctionInfo::all_matching<S: SignatureSemantics>()` finds the best-matching specializations for a call, using the provided semantics to filter and rank candidates.

Signature validation (checking that a specialized signature is a subset of the staged signature) is the caller's responsibility, typically done via `SignatureSemantics::applicable` before creating the specialization.

## Key Code Locations

- **`Signature`, `SignatureCmp`, `SignatureSemantics`** — `kirin-ir/src/signature.rs`
- **`ExactSemantics`, `LatticeSemantics`** — `kirin-ir/src/signature.rs`
- **`Dialect` trait** — `kirin-ir/src/language.rs`
- **`TypeLattice` trait** — `kirin-ir/src/lattice.rs`
- **Function model** — `kirin-ir/src/node/function.rs`
- **Builder API** — `kirin-ir/src/builder/context.rs`
