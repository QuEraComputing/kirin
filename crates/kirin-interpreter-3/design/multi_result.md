# Return Multiple Results

Multi-result operations are syntactic sugar over product types:

```
%1, %2, %3 = opA %arg0, %arg1
```

desugars to:

```
%tmp = opA %arg0, %arg1 -> tuple[int, int, int]
%1 = tuple.get %tmp, 0
%2 = tuple.get %tmp, 1
%3 = tuple.get %tmp, 2
```

## Product Container

We provide a built-in `Product` container type to model multiple results. Dialect authors build
their type systems and value types using `Product`:

```rust
enum MyType {
    I32,
    F32,
    Tuple(Product<Self>),
}

enum MyValue {
    I32(i32),
    F32(f32),
    Tuple(Product<Self>),
}
```

## ProductValue Trait

The runtime value type `V` requires a `ProductValue` bound (same as interpreter-2) which provides
all runtime operations for tuple-like values:

```rust
trait ProductValue: Sized + Clone {
    fn as_product(&self) -> Option<&Product<Self>>;
    fn from_product(product: Product<Self>) -> Self;
    fn new_product(values: Vec<Self>) -> Self;
    fn get(&self, index: usize) -> Result<Self, InterpreterError>;
    fn len(&self) -> Result<usize, InterpreterError>;
}
```

## Integration with Effects

In interpreter-3, multi-result binding is expressed via the `BindProduct` variant of the
unified `Effect` type:

```rust
Effect::BindProduct(Product<ResultValue>, V)
```

The interpreter's `consume_effect` handler auto-destructures the product value into the individual
result SSA slots using `ProductValue`. This replaces interpreter-2's `ValueStore::write_product`
method — the operation is the same, but expressed as an effect rather than a direct mutation.
