!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# SCF Dialects

The structured control flow (SCF) dialect is a dialect we adopt from the MLIR project with modifications to better fit the semantics of Python. This page will explain the SCF dialects semantics and how they are used.

## `scf.Yield`

The `scf.Yield` statement is used to mark the end of a block and yield to the region parent. It is used in the following way, for example with `scf.if` statement:

```mlir
%value_1 = scf.if %cond {
    // body
    scf.yield %value
} else {
    // body
    scf.yield %value
}
```

`scf.Yield` marks that the `%value` will be returned to the parent statement as its result. Unlike MLIR,
most of the Kirin scf dialect can also terminate with `func.Return` statement to make things easier to lower from Python.

## `scf.If`

The `scf.If` statement is used to conditionally execute a block of code. It is used in the following way:

```mlir
scf.if %cond {
    // body
} else {
    // body
}
```

**Definition** The `scf.If` statement can have a `cond` argument, a `then_body` region with single block, and optionally a `else_body` with single block. The `then_body` block is executed if the condition is true, and the `else_body` block is executed if the condition is false.

**Termination** `then_body` must terminate with `scf.Yield` or `func.Return` statement. `else_body` is optional and can be omitted. If one of the body terminates with `scf.Yield` the other body must terminate explicitly with `scf.Yield` or `func.Return`.

## `scf.For`

The `scf.For` statement is used to iterate over a range of values. It is used in the following way:

```python
def simple_loop():
    j = 0.0
    for i in range(10):
        j = j + i
    return j
```

lowered to the following IR:

```llvm
func.func simple_loop() -> !Any {
  ^0(%simple_loop_self):
  │   %j = py.constant.constant 0.0
  │   %0 = py.constant.constant IList(range(0, 10))
  │ %j_1 = py.constant.constant 45.0
  │ %j_2 = scf.for %i in %0
  │        │ iter_args(%j_3 = %j) {
  │        │ %j_4 = py.binop.add(%j_3, %i)
  │        │        scf.yield %j_4
  │        }
  │        func.return %j_1
} // func.func simple_loop
```

**Definition** The [`scf.For`][kirin.dialects.scf.For] statement takes an `iterable` as an argument.

!!! note
    Unlike MLIR where the loop iterable is restricted to a step range, Kirin allows any Python iterable object to be used as a loop iterable by marking this iterable argument as `ir.types.Any`. While it can be any Python iterable object, the actual loop compilation can only happen if the iterable type is known and supported by the compiler implementation.

[`scf.For`][kirin.dialects.scf.For] can also take an optional `initializers` tuple of values that are used to initialize the loop variables (printed as right-hand side of the `iter_args` field).

**Termination** The loop body must terminate with `scf.Yield` or `func.Return` statement.

**Scoping** The loop body creates a new scope. As a result of this, any variables defined inside the loop body are not accessible outside the loop body unless they are explicitly yielded.

!!! warning "Known difference with Python `for` loop"
    The [`scf.For`][kirin.dialects.scf.For] statement does not follow exactly the same semantics as Python `for` loop. This difference is due to the context difference of compilation vs. interpretation. Like many other compiled languages, the loop body introduces a new scope and the loop variable is not accessible outside the loop body, e.g the following code will error in Julia:

    ```julia
    function simple_loop()
        for i in 1:10
            j = j + i
            if j > 5
                return j
            end
        end
        return j
    end
    ```
    will error with `UndefVarError`:

    ```julia
    julia> simple_loop()
        ERROR: UndefVarError: `j` not defined in local scope
        Suggestion: check for an assignment to a local variable that shadows a global of the same name.
        Stacktrace:
            [1] simple_loop()
            @ Main ./REPL[1]:3
            [2] top-level scope
            @ REPL[2]:1
    ```

    However, in Python this code will work due to the fact that interpreter will not actually create a new scope for the loop body:

    ```python
    def simple_loop():
        for i in range(10):
            j = j + i
            if j == 5:
                return j
        return j # will refer to the j defined in the loop body
    ```

## Reference

::: kirin.dialects.scf.stmts
    options:
        show_if_no_docstring: true
