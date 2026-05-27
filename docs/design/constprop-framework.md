# Constant Propagation Framework

This design extracts the toy-language `ConstProp` analysis into a reusable
`kirin-constprop` crate built on top of `kirin-interpreter-new`.

The toy implementation proved the interpreter integration points:

- dialect statements can reuse their existing `Interpretable` impls when the
  abstract value implements arithmetic, comparison, branch, product, and loop
  capabilities;
- `StandardFixpointInterpreter` is the right driver for owner-summary
  convergence;
- dialect-specific convergence boundaries, such as `scf.for`, should remain
  owner semantics rather than becoming special cases in the frame protocol.

The main limitation is that the toy value lattice uses one recursive
`Product(Self)` variant for tuple-like data. That makes ordinary multi-result
plumbing and user-level aggregate values look the same, and it leaves no place
to model richer aggregate shapes.

## Crate Shape

`kirin-constprop` owns the reusable abstract value domain and a constant
propagation interpreter shell over the existing abstract fixpoint driver. It
should not own language-specific function lookup, cross-stage call resolution,
or stage enum dispatch.

Initial modules:

- `value`: reusable lattice elements and interpreter capability impls.
- `shell`: constprop-specific marker traits and aliases for specializing
  dialect interpretation against this analysis.
- `summary`: reusable function and generic IR-location owner/summary shapes for
  the standard owner-local constprop fixpoint.
- `semantics`: optional builders for common owner-semantics patterns after the
  toy implementation is migrated.

`mod.rs` files should stay declarative. Substantial logic belongs in sibling
files, following the repository module convention.

## Value Domain

The initial value domain is:

```rust
pub enum ConstPropValue<C = i64, S = String, F = String> {
    Bottom,
    Const(C),
    PartialTuple(Box<PartialTuple<Self>>),
    PartialStruct(Box<PartialStruct<S, F, Self>>),
    Top,
}
```

`PartialTuple` is the aggregate value that replaces the toy implementation's
`Product` variant. It still exposes `HasProductValue` through `Product<Self>`
because the interpreter framework uses `Product<V>` as the packet for
arguments, returns, yields, and block transfers. The distinction is semantic:
`Product<V>` remains transport, while `PartialTuple<V>` is a user-visible
abstract value.

`PartialStruct<S, F, V>` models user-defined aggregate shapes. The `S` parameter
identifies the struct shape and `F` identifies fields. A join between two
partial structs is field-wise only when the shape and ordered field keys match.
Different shapes join to `Top` and meet to `Bottom`.

The scalar parameter `C` keeps the framework open to non-`i64` constants. The
first implementation provides the existing toy semantics for `i64`, including
arithmetic, bitwise, comparison, branch truthiness, `scf.for` induction values,
and conversion from `ArithValue`.

## Lattice Semantics

The lattice is finite for a fixed aggregate shape:

- `Bottom` means unreachable or uninitialized.
- `Const(c)` means exactly one known scalar constant.
- `PartialTuple` means a known tuple shape with field-wise abstract values.
- `PartialStruct` means a known struct shape with field-wise abstract values.
- `Top` means unknown value, unknown shape, or a disagreement that cannot be
  represented more precisely.

Join rules:

- `Bottom join x = x`.
- equal constants join to that constant.
- tuple and struct values with the same shape join field-wise.
- mismatched constants, aggregate shapes, or aggregate kinds join to `Top`.

Meet follows the dual rules, with mismatches meeting to `Bottom`.

## Interpreter Integration

The value domain implements the capabilities that existing dialect
`interpreter_new` modules already expect:

- `HasProductValue` wraps interpreter products as `PartialTuple`;
- `BranchCondition` for `i64` constants uses zero/non-zero truthiness;
- arithmetic and bitwise traits compute constants when all inputs are constants,
  propagate `Bottom`, and otherwise return `Top`;
- `CompareValue` returns `Const(1)` or `Const(0)` when comparison inputs are
  known;
- dialect crates can add analysis-specific capabilities, such as SCF loop
  induction semantics, without making `kirin-constprop` depend on those
  dialects.

This keeps dialect semantics capability-based. Dialects should not pattern
match directly on `ConstPropValue` unless they are implementing aggregate
operations specific to constant propagation.

The shell layer exposes:

```rust
pub type ConstPropFixpointInterpreter<...> =
    StandardFixpointInterpreter<...>;

pub trait ConstPropDomain: AbstractValue + HasProductValue {}

pub trait ConstPropInterpreterShell<V>: Env<V> {}
```

The alias makes the intended analysis shell explicit while still using the
standard fixpoint interpreter implementation. The marker trait gives dialect
authors a stable specialization hook:

```rust
impl<L, I, F, C, E, X> Interpretable<L, I, F, C, E, X> for MyDialectOp
where
    L: Dialect,
    I: ConstPropInterpreterShell<X::Value, Error = E>,
    X: BlockTransfer,
    X::Value: ConstPropDomain,
{
    // constprop-specific transfer function
}
```

This is intentionally a trait-based shell instead of a newtype wrapper. The
standard frames already implement `Frame` against `StandardFixpointInterpreter`;
a wrapper would need to mirror the driver and all dispatch traits just to regain
that behavior.

## Fixpoint Policy

The crate should not replace `StandardFixpointInterpreter`. It layers reusable
owner/summary helpers over it.

The reusable owner key is:

```rust
pub enum ConstPropOwner {
    Function(ConstPropFunctionOwner),
    Location(Location),
}
```

The reusable summary is:

```rust
pub enum ConstPropSummary<V, L = ()> {
    Function(ConstPropFunctionSummary<V>),
    Location(Option<L>),
}
```

`ConstPropSummary<V, L>` implements `Summary`, so language integrations do not
need to rewrite the field-wise function-return merge logic or the common
owner-summary shell. The location payload `L` is supplied by the dialect or
analysis integration through `ConstPropLocationSummary<V>`. This keeps
`kirin-constprop` independent of control-flow structure while still giving
dialects a reusable summary slot for IR locations that need local convergence.

SCF-specific support lives in `kirin-scf` behind the `constprop` feature, where
that crate implements:

- `ForLoopValue` for `ConstPropValue<i64, _, _>`;
- `ScfForConstPropSummary<V>` as the location payload for `scf.for`;
- `ScfForFixpointSummary<..., ConstPropOwner>` for
  `ConstPropSummary<V, ScfForConstPropSummary<V>>`.

Language integrations still own the parts that are necessarily
language-specific: resolving call targets, choosing a root frame for a stage,
and converting completions into summary updates.

Recommended migration path:

1. Use `ConstPropValue` for the analysis lattice.
2. Use `ConstPropOwner` and `ConstPropSummary<V, L>` for standard function and
   location convergence summaries.
3. Enable the dialect crate's constprop support for dialect-specific semantics,
   such as `kirin-scf = { features = ["constprop"] }`.
4. Keep toy-language call-target resolution and stage dispatch in
   `example/toy-lang`, because those are language-specific.
5. Keep `scf.for` convergence owner-based through `ScfForFixpointSummary`, not
   as a special case in the generic driver.

This gives a reusable abstract value and owner-summary shell while keeping
control-flow-specific convergence payloads in the dialect crates that define
those control-flow forms.
