# Multi-Result Values

This document specifies the design for consistent multi-result support across the
kirin framework: IR definitions, text format, parser, printer, interpreter, and
abstract interpreter.

## Motivation

Kirin operations can produce multiple SSA values (dataflow edges), but the framework
only partially supports this. The derive builder rejects `Vec<ResultValue>` and
`Option<ResultValue>` fields. SCF operations like `If` and `For` are limited to
one result, and `For` supports only a single loop-carried accumulator.

This design removes those limitations while keeping the interpreter single-valued.

## Core Insight: Multi-Result IS a Product Type

Multi-result and tuple are formally the same thing — both are product types. Having
both as separate mechanisms at the interpreter level is duplicated logic. MLIR's own
experience confirms this: the community uses multi-result instead of tuples, and
the builtin `tuple` type is widely regarded as vestigial.

Kirin's design unifies them: **multi-result is syntactic sugar over product types**.

- The **IR representation** keeps multi-result fields (`Vec<ResultValue>`) — the IR
  faithfully represents what the user wrote.
- The **text format** keeps multi-result syntax (`%a, %b = op ... -> (T1, T2)`) —
  no desugaring at parse time.
- The **interpreter** is single-valued — `Yield(V)`, `Return(V)` carry one value.
  When a statement has multiple results, the returned V is a product. The framework
  auto-destructures it into individual result slots.
- The **abstract interpreter** needs no special multi-result handling — the product
  is a single lattice element in the value domain.

## ProductType: The Foundation

`ProductType<T>` is a generic wrapper in `kirin-ir` that represents an ordered
collection of types. It uses `SmallVec` for compact storage (most products have
1-3 elements).

```rust
// In kirin-ir:
use smallvec::SmallVec;

/// An ordered product of types — the type-level representation of
/// multi-result operations and tuple values.
///
/// Uses SmallVec for compact storage: products with 1-2 elements
/// avoid heap allocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProductType<T>(pub SmallVec<[T; 2]>);
```

### HasProduct Trait

Dialect types that support product types implement `HasProduct`. This is opt-in —
only dialects that want multi-result statements need it.

```rust
/// Dialect types that support product types implement this.
///
/// The framework uses this to:
/// - Parse `(T1, T2)` syntax as product types
/// - Print product types
/// - Auto-destructure multi-result statements during interpretation
///
/// Similar to `Placeholder` — an opt-in trait that derive macros detect
/// and add bounds for automatically.
pub trait HasProduct: Sized {
    /// Wrap element types into a product type.
    fn product(elements: SmallVec<[Self; 2]>) -> Self;

    /// Extract element types if this is a product. Returns None otherwise.
    fn as_product(&self) -> Option<&[Self]>;
}
```

### Dialect Author Usage

```rust
use kirin_ir::ProductType;

#[derive(Debug, Clone, PartialEq, Eq, Hash, HasParser, PrettyPrint)]
enum MyType {
    I32,
    F64,
    Tuple(ProductType<MyType>),  // user chooses the variant name
}

impl HasProduct for MyType {
    fn product(elements: SmallVec<[Self; 2]>) -> Self {
        MyType::Tuple(ProductType(elements))
    }
    fn as_product(&self) -> Option<&[Self]> {
        match self {
            MyType::Tuple(p) => Some(&p.0),
            _ => None,
        }
    }
}
```

The dialect author chooses the variant name (`Tuple`, `Product`, `Struct`, etc.)
— the framework recognizes it through the `HasProduct` trait, not by name.

### Derive Detection

When a dialect struct has `Vec<ResultValue>` fields and `#[kirin(builders)]`, the
derive macro automatically adds `T: HasProduct` to the generated builder's `where`
clause — the same pattern as `T: Placeholder` for auto-placeholder. The dialect
author never writes `+ HasProduct` on their struct definitions.

## ProductValue Trait

The interpreter-level counterpart of `HasProduct`. Dialect value types implement
this to define how product values are constructed and destructured at runtime.

```rust
/// Interpreter-level product value semantics.
///
/// Dialect authors implement this on their value types to enable
/// multi-result statements and tuple operations.
pub trait ProductValue: Sized {
    /// Pack multiple values into a single product value.
    fn new_product(values: Vec<Self>) -> Self;

    /// Unpack a product value into its component values.
    fn unpack(self) -> Result<Vec<Self>, InterpreterError>;

    /// Extract a single element by index (does not consume the product).
    fn get(&self, index: usize) -> Result<Self, InterpreterError>;

    /// Query the number of elements.
    fn len(&self) -> Result<usize, InterpreterError>;

    /// Returns true if the product has zero elements.
    fn is_empty(&self) -> Result<bool, InterpreterError> {
        self.len().map(|n| n == 0)
    }

    /// Convert a value to a usize index (for Get statement).
    fn as_index(&self) -> Result<usize, InterpreterError>;

    /// Create a value from a usize (for Len statement).
    fn from_index(index: usize) -> Self;
}
```

## Continuation Enum (Unchanged)

The `Continuation` enum stays **single-valued**. No `SmallVec` wrapping.

```rust
pub enum Continuation<V, Ext = Infallible> {
    Continue,
    Jump(Block, Args<V>),
    Fork(SmallVec<[(Block, Args<V>); 2]>),
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Args<V>,
        result: ResultValue,
    },
    Return(V),
    Yield(V),
    Ext(Ext),
}
```

When a function returns multiple values, `V` is a product. When a SCF body yields
multiple values, `V` is a product. The framework auto-destructures based on the
IR's `Vec<ResultValue>` fields.

## Auto-Destructuring: Statement Execution Layer

When the framework executes a statement with multiple results (`Vec<ResultValue>`
with N > 1), it treats the returned value as a product and writes each element
to the corresponding result slot:

```rust
// Pseudocode — in the statement execution layer:
fn write_statement_results<V: ProductValue>(
    store: &mut impl ValueStore<Value = V>,
    results: &[ResultValue],
    value: V,
) -> Result<(), Error> {
    if results.len() <= 1 {
        // Single result — write directly (common case, no product overhead)
        if let Some(rv) = results.first() {
            store.write(*rv, value)?;
        }
    } else {
        // Multi-result — auto-destructure product
        for (i, rv) in results.iter().enumerate() {
            let element = value.get(i)?;
            store.write(*rv, element)?;
        }
    }
    Ok(())
}
```

This happens in the framework, not in dialect interpret impls. The dialect author's
`interpret()` method returns a single value (possibly a product), and the framework
handles the rest.

## Abstract Interpreter

No changes needed. `AnalysisResult` stays `Option<V>`. If `V` is a product, the
lattice handles it internally — join, meet, widening, narrowing all operate on
the single `V`. The product lattice `L^n` is encoded in the value domain, not
the framework.

```rust
// AnalysisResult — unchanged:
pub struct AnalysisResult<V> {
    pub return_value: Option<V>,
}
```

When two paths return products of different structure, the lattice's `join`
implementation handles it (e.g., widening to Top, or pointwise join if the
product arity matches). This is the dialect author's responsibility via their
`AbstractValue` impl.

## Derive Builder Changes

The builder template in `kirin-derive-toolkit` supports `Vec<ResultValue>` and
`Option<ResultValue>` fields. When `Vec<ResultValue>` is present, the derive
automatically adds `T: HasProduct` to the builder's `where` clause.

## Text Format

### Result Types

Multi-result types are printed as product types `(T1, T2)` via `HasProduct`:

```
%a, %b = my_op %x -> (i32, f64)
```

The parser recognizes `(T1, T2)` and calls `HasProduct::product(vec![T1, T2])`.

### Format DSL `[...]` Optional Sections

The `[...]` syntax in format strings marks optional groups for void operations:

```rust
#[chumsky(format = "$if {cond} then {then_body} else {else_body}[ -> {result:type}]")]
```

### Multi-Value Yield and Return

```
yield %a, %b       // sugar: yields product of %a and %b
ret %a, %b          // sugar: returns product of %a and %b
```

The SCF `Yield` and function `Return` statements still have `Vec<SSAValue>` fields
in the IR. At interpretation time, the interpret impl packs them into a product
via `ProductValue::new_product` and returns a single `Yield(product)` /
`Return(product)`.

## kirin-tuple Dialect

The `kirin-tuple` crate provides explicit tuple operations for dialect authors who
want to work with product values in their DSL programs:

| Statement | Description |
|-----------|-------------|
| `new_tuple(%a, %b) -> T` | Pack SSA values into a product |
| `unpack %t -> T1, T2` | Bulk destructure (arity known) |
| `get %t, %idx -> T` | Extract one element by index (arity not required) |
| `len %t -> T` | Query arity |

The dialect uses `ProductValue` (renamed from `TupleValue`) for interpreter
semantics. Dialect authors who only use multi-result syntax don't need to depend
on `kirin-tuple` — the `ProductValue` trait and auto-destructuring are framework-level.

## Summary: What Lives Where

| Component | Location | Purpose |
|-----------|----------|---------|
| `ProductType<T>` | kirin-ir | Type-level product wrapper |
| `HasProduct` | kirin-ir | Trait for dialect types — opt-in multi-result |
| `ProductValue` | kirin-interpreter | Trait for dialect values — product semantics |
| Auto-destructure | kirin-interpreter | Statement execution writes product to result slots |
| `Tuple` dialect | kirin-tuple | Explicit pack/unpack/get/len operations |
| `Vec<ResultValue>` support | kirin-derive-toolkit | Builder template codegen |
| `[...]` syntax | kirin-derive-chumsky | Optional sections in format strings |

## References

- [Rationale for not having tuple type operations](https://discourse.llvm.org/t/rationale-for-not-having-tuple-type-operations-in-the-main-dialects/3748)
- [Rationale behind MLIR's builtin tuple type](https://discourse.llvm.org/t/rationale-behind-mlirs-builtin-tuple-type/84424)
- Cousot, P. & Cousot, R. (1977). "Abstract interpretation: a unified lattice model."
