!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.


# The Immutable List Dialect

The immutable list dialect models a Python-like `list` but is immutable. There are many good reasons to have an immutable list for compilation, especially when a list is immutable, we can assume a lot of statements to be pure and foldable (and thus can be inlined or simplified without extra analysis/rewrites).

!!! note "JAX"
    This is also the reason why JAX picks an immutable array semantic.

!!! note "Why not use tuple?"
    Tuple can take multiple items of different types, but list can only take items of the same type. Thus they have different trade-offs when doing analysis such as type inference of iterating through a tuple/list.

## Runtime

This dialect provides a runtime object `IList` which is a simple Python class wraps a Python list. This object can be used as a compile-time value by providing an implementation of `__hash__` that returns the object id. This means common simplifications like Common Subexpression Elimination (CSE) will not detect duplicated `IList` unless the `ir.SSAValue` points to identical `IList` object.

!!! warning "Implementation Details"
    this is an implementation detail of `IList`, we can switch to a more efficient runtime in the future where the memory layout is optimized based on the assumption of items in same type and immutabiility.

The `IList` runtime object implements most of the Python `Sequence` interface, such as `__getitem__`, `__iter__`, `__len__` etc.

## `New`

This statements take a tuple of [`ir.SSAValue`][kirin.ir.SSAValue] and creates an `IList` as result.

!!! note "Syntax Sugar in Lowering"
    The syntax `[a, b, c]` will be lowered into `New` statement as a syntax sugar when `ilist` dialect is used (thus in conflict with mutable Python list). This may change in the future to give developers more freedom to choose what to lower from.

## `Map`

This statements take a high-order function (a function object) of signature `[[ElemT], OutT]` and apply it on a given `IList[ElemT, Len]` object then return a new `IList[OutT, Len]`.

For example:

```python
@basic_no_opt
def main(x: ilist.IList[float, Literal[5]]):
    def closure(a):
        return a + 1
    return ilist.map(closure, x)
```

will be lowerd into the following

```mlir
func.func main(!py.IList[!py.float, 5]) -> !Any {
  ^0(%main_self, %x):
  │ %closure = func.lambda closure() -> !Any {
  │            │ ^1(%closure_self, %a):
  │            │ │ %1 = py.constant.constant 1 : !py.int
  │            │ │ %2 = py.binop.add(%a, %1) : ~T
  │            │ │      func.return %2
  │            } // func.lambda closure
  │       %0 = py.ilist.map(fn=%closure, collection=%x : !py.IList[!py.float, 5]) : !py.IList[~OutElemT, ~ListLen]
  │            func.return %0
} // func.func main
```

## `Foldl` and `Foldr`

These two statements represents applying a binary operator `+` (any binary operator) on an `IList` with a different reduction order, e.g given `[a, b, c]`, `Foldl` represents `((a + b) + c)` and `Foldr` represents `(a + (b + c))`.

## `Scan`

While the actual implementation is not the same, this statement represents the same semantics as the following function:

```python
def scan(fn, xs, init):
    carry = init
    ys = []
    for elem in xs:
        carry, y = fn(carry, elem)
        ys.append(y)
    return carry, ys
```

## `ForEach`

this represents a `for`-loop without any loop variables (variables pass through each loop iteration).

## API Reference

::: kirin.dialects.ilist.stmts
    options:
        show_if_no_docstring: true
