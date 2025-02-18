!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# The Function Dialect

The function dialect provides a set of statements to model semantics of Python-like functions, that means:

- `def <name>(<args>*, <kwargs>*)` like function declarations
- nested functions (namely closures)
- high-order functions (functions can be used as arguments)
- dynamically/statically calling a function or closure

## `func.Return`

This is a simple statement that models the `return` statement in a function declaration. While this is a very simple statement, it is worth noting that this statement only accepts **one argument** of type [ir.SSAValue][kirin.ir.SSAValue] because in Python (and most of other languages) functions always have a single return value, and multiple return values are represented by returning a `tuple`.

## `func.Function`

This is the most fundamental statement that models a Python function.

**Definition** The [`func.Function`][kirin.dialects.func.Function] takes no arguments, but contains a special `str` attribute (thus stored as `PyAttr`) that can be used as a symbolic reference within a symbol table. The `func.Function` also takes a `func.Signature` attribute to store the signature of corresponding function declaration. Last, it contains a [`ir.Region`][kirin.ir.Region] that represents the function body. The [`ir.Region`][kirin.ir.Region] follows the SSACFG convention where the blocks in the region forms a control flow graph.

!!! note "Differences with MLIR"
    As Kirin's priority is writing eDSL as kernel functions in Python. To support high-order functions the entry block arguments always have their first argument `self` of type [`types.MethodType`][kirin.types.MethodType]. This is a design inspired by [Julia](https://julialang.org)'s IR design.

As an example, the following Python function

```python
from kirin.prelude import basic_no_opt

@basic_no_opt
def main(x):
    return x
```

will be lowered into the following IR, where `main_self` referencing the function itself.

```mlir
func.func main(!Any) -> !Any {
  ^0(%main_self, %x):
  │ func.return %x
} // func.func main
```

the function can be terminated by a [`func.Return`][kirin.dialects.func.Return] statement. All blocks in the function region must have terminators. In the lowering process, if the block is not terminated, a `func.Return` will be attached to return `None` in the function body. Thus `func.Function` can only have a single return value.

## `func.Call` and `func.Invoke`

These two statements models the most common call convention in Python with consideration of compilation:

- `func.Call` models dynamic calls where the **callee is unknown at compile time**, thus of type [`ir.SSAValue`][kirin.ir.SSAValue]
- `func.Invoke` models static calls where the **callee is known at compile time**, thus of type [`ir.Method`][kirin.ir.Method]

they both take `inputs` which is a tuple of [`ir.SSAValue`][kirin.ir.SSAValue] as argument. Because we assume all functions will only return a single value, `func.Call` and `func.Invoke` only have a single result.

## `func.Lambda`

This statement models nested functions (a.k.a closures). While most definitions are similar to `func.Function` the key difference is `func.Lambda` takes a tuple of [`ir.SSAValue`][kirin.ir.SSAValue] arguments as `captured`. This models the captured variables for a nested function, e.g

the following Python function containing a closure inside with variable `x` being captured:

```python
from kirin import basic_no_opt

@basic_no_opt
def main(x):
    def closure():
        return x
    return closure
```

will be lowered into

```mlir
func.func main(!Any) -> !Any {
  ^0(%main_self, %x):
  │ %closure = func.lambda closure(%x) -> !Any {
  │            │ ^1(%closure_self):
  │            │ │ %x_1 = func.getfield(%closure_self, 0) : !Any
  │            │ │        func.return %x_1
  │            } // func.lambda closure
  │            func.return %closure
} // func.func main
```

Unlike `func.Function` this statement also has a result value which points to the closure itself. Inside the closure body, we insert [`func.GetField`][kirin.dialects.func.GetField] to unpack captured variables into the closure body.

## API Reference

::: kirin.dialects.func.stmts
    options:
        show_if_no_docstring: true
