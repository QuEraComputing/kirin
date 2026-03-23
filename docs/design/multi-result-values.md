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

## Product\<T\>: Unified Storage

A single `Product<T>` in `kirin-ir` serves both the type level and the value level:

- `Product<MyType>` — a product of types (in dialect type enums)
- `Product<MyValue>` — a product of values (in dialect value enums)

There is no separate `ProductType` or `ProductValue` struct — one generic
wrapper handles both.

```rust
// In kirin-ir:
use smallvec::SmallVec;

/// An ordered product — used for both type-level products (multi-result
/// type annotations) and value-level products (packed interpreter values).
///
/// Uses SmallVec for compact inline storage: products with ≤2 elements
/// avoid heap allocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Product<T>(pub SmallVec<[T; 2]>);
```

### Construction Macro

The `product!` macro mirrors `smallvec!` for ergonomic construction:

```rust
/// Construct a `Product` inline.
///
/// ```rust
/// let types = product![I32, F64];       // Product<MyType>
/// let values = product![v1, v2, v3];    // Product<MyValue>
/// let empty = product![];               // Product<T> (empty)
/// ```
#[macro_export]
macro_rules! product {
    ($($elem:expr),* $(,)?) => {
        $crate::Product(smallvec::smallvec![$($elem),*])
    };
}
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
    /// Wrap a product into this type.
    fn from_product(product: Product<Self>) -> Self;

    /// Extract the product if this is a product type. Returns None otherwise.
    fn as_product(&self) -> Option<&Product<Self>>;
}
```

### Dialect Author Usage (Types)

```rust
use kirin_ir::Product;

#[derive(Debug, Clone, PartialEq, Eq, Hash, HasParser, PrettyPrint)]
enum MyType {
    I32,
    F64,
    Tuple(Product<MyType>),  // user chooses the variant name
}

impl HasProduct for MyType {
    fn from_product(product: Product<Self>) -> Self {
        MyType::Tuple(product)
    }
    fn as_product(&self) -> Option<&Product<Self>> {
        match self {
            MyType::Tuple(p) => Some(p),
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

### 2 Required Methods

The `ProductValue` trait requires only 2 methods. All product operations
are provided as defaults. `unpack` returns a borrowing iterator — the
caller decides when to clone.

```rust
/// Interpreter-level product value semantics.
///
/// Uses the same `Product<T>` from kirin-ir. 2 required methods,
/// all operations provided. No unnecessary allocation.
pub trait ProductValue: Sized + Clone {
    /// Borrow the product storage if this value is a product.
    fn as_product(&self) -> Option<&Product<Self>>;

    /// Wrap a product into this value type.
    fn from_product(product: Product<Self>) -> Self;

    // --- All provided ---

    fn new_product(values: Vec<Self>) -> Self {
        Self::from_product(Product(SmallVec::from_vec(values)))
    }

    /// Extract one element by index (clones the element).
    fn get(&self, index: usize) -> Result<Self, InterpreterError> {
        self.as_product()
            .and_then(|p| p.0.get(index).cloned())
            .ok_or_else(|| InterpreterError::Custom(
                format!("product index {index} out of bounds").into()
            ))
    }

    fn len(&self) -> Result<usize, InterpreterError> {
        self.as_product()
            .map(|p| p.0.len())
            .ok_or_else(|| InterpreterError::Custom("expected product".into()))
    }

    fn is_empty(&self) -> Result<bool, InterpreterError> {
        self.len().map(|n| n == 0)
    }
}
```

No `unpack` method or `UnpackIter` type — `Product<T>` already implements
`IntoIterator` and `iter()`. Users who need to iterate over elements use
`as_product()` directly:

```rust
let product = value.as_product().ok_or(err)?;
for elem in product {       // &Product<V> -> IntoIterator -> &V
    // process &V
}
```
```

### Product\<T\> — Iterators

`Product<T>` delegates to `SmallVec` iterators for zero-allocation traversal:

```rust
impl<V> Product<V> {
    pub fn iter(&self) -> core::slice::Iter<'_, V> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, V> {
        self.0.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<V> IntoIterator for Product<V> {
    type Item = V;
    type IntoIter = smallvec::IntoIter<[V; 2]>;
    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}

impl<'a, V> IntoIterator for &'a Product<V> {
    type Item = &'a V;
    type IntoIter = core::slice::Iter<'a, V>;
    fn into_iter(self) -> Self::IntoIter { self.0.iter() }
}

impl<V> FromIterator<V> for Product<V> {
    fn from_iter<I: IntoIterator<Item = V>>(iter: I) -> Self {
        Product(iter.into_iter().collect())
    }
}
```

All destructuring uses `as_product()` + `Product<T>` iterators — no
wrapper types, no extra allocation:

```rust
// Auto-destructure layer — borrows, clones per element:
let product = value.as_product().ok_or(err)?;
for (rv, elem) in results.iter().zip(product) {
    store.write(*rv, elem.clone())?;
}

// kirin-tuple Unpack — same pattern:
let product = source.as_product().ok_or(err)?;
for (rv, elem) in self.results.iter().zip(product) {
    interp.write(*rv, elem.clone())?;
}

// Single element access:
let elem = value.get(0)?;  // clones one element

// Owning destructure when needed — clone then consume:
let owned = value.as_product().ok_or(err)?.clone();
for elem in owned {  // Product<V>: IntoIterator yields V (owned)
    // process V
}
```

### Dialect Author Usage — Minimal

Both the type enum and value enum use the same `Product<T>` from kirin-ir:

```rust
use kirin_ir::Product;

// Type enum — same Product<T> as values:
enum MyType {
    I32, F64,
    Tuple(Product<MyType>),
}

// Value enum — same Product<T> as types:
enum MyValue {
    Int(i64),
    Float(f64),
    Tuple(Product<MyValue>),
}

// HasProduct (type level) — 2 methods:
impl HasProduct for MyType {
    fn from_product(p: Product<Self>) -> Self { MyType::Tuple(p) }
    fn as_product(&self) -> Option<&Product<Self>> {
        match self { MyType::Tuple(p) => Some(p), _ => None }
    }
}

// ProductValue (value level) — 2 methods:
impl ProductValue for MyValue {
    fn as_product(&self) -> Option<&Product<Self>> {
        match self { MyValue::Tuple(p) => Some(p), _ => None }
    }
    fn from_product(p: Product<Self>) -> Self { MyValue::Tuple(p) }
}
// Done — new_product, unpack, get, len, is_empty all free.
// unpack returns UnpackIter (borrowing) — caller clones when needed.
```

### IndexValue Trait — Separate Concern

The `Get` and `Len` operations in `kirin-tuple` need to convert between
value types and `usize` (index as an SSA value). This is a general
integer conversion concern, not specific to products:

```rust
/// Convert between interpreter values and usize indices.
/// Used by kirin-tuple's Get (value → index) and Len (index → value).
pub trait IndexValue: Sized {
    fn as_index(&self) -> Result<usize, InterpreterError>;
    fn from_index(index: usize) -> Self;
}
```

This lives in `kirin-tuple` (not the framework), since only tuple operations
need it.

### Future: Derive Macro Opportunities

Several boilerplate patterns in this design are candidates for derive macros:

| Pattern | Current | Future Derive |
|---------|---------|---------------|
| `HasProduct` impl on type enum | Manual 2-method impl | `#[derive(HasProduct)]` with `#[product]` on variant |
| `ProductValue` impl on value enum | Manual 2-method impl | `#[derive(ProductValue)]` with `#[product]` on variant |
| `IndexValue` impl on value enum | Manual 2-method impl | `#[derive(IndexValue)]` with `#[index]` on variant |
| `HasParser`/`PrettyPrint` for `Product<T>` | Manual or framework-provided | Auto-detected `(T1, T2)` syntax for product variants |
| `AbstractValue` join for products | Manual pointwise join | Derive-generated pointwise join when `#[product]` present |

Note: `HasProduct` and `ProductValue` have identical required methods (`as_product`
+ `from_product`). A single `#[product]` attribute could generate both impls.
In practice, type enums and value enums are separate, so separate derives are
cleaner — but the attribute and detection logic can be shared in derive-toolkit.

These derives are **not required for the initial implementation** — the manual
impls are 2-5 lines each. But they eliminate the last bits of boilerplate for
dialect authors who use products heavily. They should be added to the derive
infrastructure when the product pattern is validated across multiple dialects.

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

| Statement | Description | Bound |
|-----------|-------------|-------|
| `new_tuple(%a, %b) -> T` | Pack SSA values into a product | `V: ProductValue` |
| `unpack %t -> T1, T2` | Bulk destructure (arity known) | `V: ProductValue` |
| `get %t, %idx -> T` | Extract one element by index (arity not required) | `V: ProductValue + IndexValue` |
| `len %t -> T` | Query arity | `V: ProductValue + IndexValue` |

`ProductValue` lives in kirin-interpreter (framework-level). `IndexValue` lives
in kirin-tuple (dialect-level — only needed for Get/Len).

Dialect authors who only use multi-result syntax (`%a, %b = op ...`) don't need
to depend on `kirin-tuple` — the `ProductValue` trait and auto-destructuring are
framework-level. They only need `kirin-tuple` if they want explicit pack/unpack
operations in their DSL programs.

## Summary: What Lives Where

| Component | Location | Purpose |
|-----------|----------|---------|
| `Product<T>` | kirin-ir | Unified product storage for both types and values (SmallVec-backed) |
| `product![]` macro | kirin-ir | Ergonomic product construction |
| `HasProduct` | kirin-ir | Trait for dialect types — opt-in multi-result (2 required methods) |
| `ProductValue` | kirin-interpreter | Trait for dialect values — 2 required, 3 provided (`new_product`, `get`, `len`) |
| Auto-destructure | kirin-interpreter | Statement execution writes product to result slots |
| `IndexValue` | kirin-tuple | Trait for value ↔ usize conversion (Get/Len only) |
| `Tuple` dialect | kirin-tuple | Explicit new_tuple/unpack/get/len operations |
| `Vec<ResultValue>` support | kirin-derive-toolkit | Builder template codegen |
| `[...]` syntax | kirin-derive-chumsky | Optional sections in format strings |

## References

- [Rationale for not having tuple type operations](https://discourse.llvm.org/t/rationale-for-not-having-tuple-type-operations-in-the-main-dialects/3748)
- [Rationale behind MLIR's builtin tuple type](https://discourse.llvm.org/t/rationale-behind-mlirs-builtin-tuple-type/84424)
- Cousot, P. & Cousot, R. (1977). "Abstract interpretation: a unified lattice model."
