!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# The Control Flow Dialect

The control flow dialect provides the most generic control flow semantics via [`cf.Branch`][kirin.dialects.cf.Branch] and [`cf.ConditionalBranch`][kirin.dialects.cf.ConditionalBranch].

## `cf.Branch`

the [`cf.Branch`][kirin.dialects.cf.Branch] statement is used to mark how basic block branches to another basic block without condition. This represents an edge on the control flow graph (CFG).

```mlir
^1(%2):
  │   %y = py.constant.constant 1 : !py.int
  │        cf.br ^3(%y)
  ^2(%3):
  │ %y_1 = py.constant.constant 2 : !py.int
  │        cf.br ^3(%y_1)
  ^3(%y_2):
  │        func.return %y_2
```

**Definition** the `cf.Branch` statement takes a successor block and its argument. The `cf.Branch` is a terminator thus it should always be the last statement of a block.

!!! note
    [`ir.Statement`][kirin.ir.Statement] does not own any [`ir.Block`][kirin.ir.Block], the [`ir.Region`][kirin.ir.Region] owns blocks. The [`ir.Statement`][kirin.ir.Statement] will only own [`ir.Region`][kirin.ir.Region]. In Kirin, we use similar design as LLVM/MLIR where the phi nodes in SSA form are replaced by block arguments.

## `cf.ConditionalBranch`

The [`cf.ConditionalBranch`][kirin.dialects.cf.ConditionalBranch] statement represents a conditional branching statement that looks like following (the `cf.cond_br` statement):

```mlir
^0(%main_self, %x):
│   %0 = py.constant.constant 1 : !py.int
│   %1 = py.cmp.gt(lhs=%x, rhs=%0) : !py.bool
│        cf.cond_br %1 goto ^1(%1) else ^2(%1)
```

**Definition**, [`cf.ConditionalBranch`][kirin.dialects.cf.ConditionalBranch] takes a boolean condition `cond` of type [`ir.SSAValue`][kirin.ir.SSAValue] and:

- then successor and its argument
- else successor and its argument

this statement is also a terminator, which means it must be the last statement of a block.

## Combining together - lowering from Python

Now combining these two statemente together, we can represent most of the Python control flows, e.g `if-else` and `for`-loops. These two statement basically just provides a basic way describing the edges on a control flow graph (CFG) by assuming the node only has one or two outgoing edges.

As an example, the following Python program:

```python
from kirin.prelude import basic_no_opt

@basic_no_opt
def main(x):
    if x > 1:
        y = 1
    else:
        y = 2
    return y
```

will be lowered to the following SSA form in `cf` dialect:

```mlir
func.func main(!Any) -> !Any {
  ^0(%main_self, %x):
  │   %0 = py.constant.constant 1 : !py.int
  │   %1 = py.cmp.gt(lhs=%x, rhs=%0) : !py.bool
  │        cf.cond_br %1 goto ^1(%1) else ^2(%1)
  ^1(%2):
  │   %y = py.constant.constant 1 : !py.int
  │        cf.br ^3(%y)
  ^2(%3):
  │ %y_1 = py.constant.constant 2 : !py.int
  │        cf.br ^3(%y_1)
  ^3(%y_2):
  │        func.return %y_2
} // func.func main
```

And similarly, we can lower a `for`-loop into the `cf` dialect:

```python
@basic_no_opt
def main(x):
    for i in range(5):
        x = x + i
    return x
```

will be lowered into the following SSA form:

```mlir
func.func main(!Any) -> !Any {
  ^0(%main_self, %x_1):
  │ %0 = py.constant.constant 0 : !py.int
  │ %1 = py.constant.constant 5 : !py.int
  │ %2 = py.constant.constant 1 : !py.int
  │ %3 = py.range.range(start=%0, stop=%1, step=%2) : !py.range
  │ %4 = py.iterable.iter(value=%3) : !Any
  │ %5 = py.constant.constant None : !py.NoneType
  │ %6 = py.iterable.next(iter=%4) : !Any
  │ %7 = py.cmp.is(lhs=%6, rhs=%5) : !py.bool
  │      cf.cond_br %7 goto ^2(%x_1) else ^1(%6, %x_1)
  ^1(%i, %x_2):
  │ %x = py.binop.add(%x_2, %i) : ~T
  │ %8 = py.iterable.next(iter=%4) : !Any
  │ %9 = py.cmp.is(lhs=%8, rhs=%5) : !py.bool
  │      cf.cond_br %9 goto ^2(%x) else ^1(%8, %x)
  ^2(%x_3):
  │      func.return %x_3
} // func.func main
```

However, as you may already notice, lowering from Python directly to `cf` dialect will lose some of the high-level information such as the control flow is actually a for-loop. This information can be useful when one wants to perform some optimization. This is why we are taking the same route as MLIR with a structural IR (via [`ir.Region`][kirin.ir.Region]s). For the interested readers, please proceed to [Structural Control Flow](scf.md) for further reading.

## API Reference

::: kirin.dialects.cf.stmts
    options:
        show_if_no_docstring: true
