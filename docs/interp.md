!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# Interpretation

Kirin provides a framework for interpreting the IR. There are multiple ways to interpret the IR:

1. concrete interpretation, which evaluates the IR using concrete values like CPython.
2. abstract interpretation, which evaluates the IR on lattice values. (See also [Analysis](/analysis))
3. tree walking, which walks the IR tree and performs actions on each node. (See also [Code Generation](/codegen))

this page will focus on concrete interpretation.

## Concrete Interpretation

### Function-call like interpretation

The concrete interpreter is essentially a dispatcher of implementations for each statement in the IR. For dialect developers, the main task is to implement a method table, taking the `py.binop` dialect as an example:

```python
from kirin import interp # (1)!

from . import stmts # (2)!
from ._dialect import dialect # (3)!

@dialect.register # (4)!
class PyMethodTable(interp.MethodTable): # (4)!

    @interp.impl(stmts.Add) # (5)!
    def add(self, interp, frame: interp.Frame, stmt: stmts.Add): # (6)!
        return (frame.get(stmt.lhs) + frame.get(stmt.rhs),) # (7)!
```

1. Import the `interp` module.
2. Import the statements module. This is defined similarly as [Declaring Statements](/def/#defining-a-statement).
3. Import the dialect object. This is defined in a similar way as [Declaring Dialect](/def/#dialect).
4. Register the method table with the dialect. This will push the method table to the dialect's registry. By default this will be registered under the key `"main"`, equivalent to `@dialect.register(key="main")`.
5. Mark the method as an implementation of the `Add` statement. This can be dispatched on the type of the statement, e.g to only mark the implementation for `Add(Int, Int)`, you can write `@interp.impl(stmts.Add, types.Int, types.Int)`, where `types` can be imported by `from kirin import types`.
6. While this is enforced, it is recommended to type hint the frame and the statement so you can get the hinting from the IDE. The `@interp.impl` decorator will also type check if the method signature is correct.
7. In the actual implementation, [`frame.get`][kirin.interp.Frame.get] is used to get the value of the operands. This will return the value of the operand if it is defined in the frame, otherwise it will raise an [`InterpreterError`][kirin.exceptions.InterpreterError]. Most of the case, the return value should be a `tuple` of the results of the statement. In this case, there is only one result, so it is returned as a single-element tuple.

!!! note "What is a frame?"
    A frame is a mapping of [`SSAValue`][kirin.ir.SSAValue] to their actual values. It represents the state of a function-like statement that cuts the scope of the variables. The frame is passed to the method table so that the interpreter can get the values of the operands from current frame.

### Control flow statements

Except these "normal" statements that act more like a function call, there are also control flow statements. For example, the `Branch` statement from `cf` dialect, defined as follows (see also [Declaring Statements](/def/#defining-a-statement)):

```python
@statement(dialect=dialect)
class Branch(Statement):
    name = "br"
    traits = frozenset({IsTerminator()})

    arguments: tuple[SSAValue, ...]
    successor: Block = info.block()
```

When interpreting a `Branch` statement, instead of actually executing something, we would like to instruct
the interpreter to jump to a successor block. This is done by returning a special value [`interp.Successor`][kirin.interp.Successor]:

```python
@dialect.register
class CfMethods(MethodTable):

    @impl(Branch)
    def branch(self, interp: Interpreter, frame: Frame, stmt: Branch):
        return Successor(stmt.successor, *frame.get_values(stmt.arguments))
```

Similar to [`frame.get`][kirin.interp.Frame.get], [`frame.get_values`][kirin.interp.Frame.get_values] is a convenience method to get the values of multiple operands at once.

!!! note "What is a successor?"
    A successor is a tuple of a block and the values to be passed to the block. The interpreter will use this information to jump to the block and pass the values to the block.

Another special control flow statement is [`ReturnValue`][kirin.interp.ReturnValue], unlike [`interp.Successor`][kirin.interp.Successor] that jumps to another block, [`ReturnValue`][kirin.interp.ReturnValue] will let interpreter pop the current frame and return the values to the caller or finish the execution:

```python
@dialect.register
class FuncMethods(MethodTable):

    @impl(Return)
    def return_(self, interp: Interpreter, frame: Frame, stmt: Return):
        return interp.ReturnValue(*frame.get_values(stmt.values))
```

### Error handling

Some statements will throw a runtime error, such as `cf.Assert` from the `cf` dialect, defined as follows:

```python
@statement(dialect=dialect)
class Assert(Statement):
    name = "assert"
    condition: SSAValue
    message: SSAValue = info.argument(String)
```

When interpreting an `Assert` statement, we need to check the condition and raise an error if it is false:

```python
@dialect.register
class CfMethods(MethodTable):

    @impl(Assert)
    def assert_stmt(self, interp: Interpreter, frame: Frame, stmt: Assert):
        if frame.get(stmt.condition) is True:
            return ()

        if stmt.message:
            raise interp.WrapException(AssertionError(frame.get(stmt.message)))
        else:
            raise interp.WrapException(AssertionError("Assertion failed"))
```

or raising an [`InterpreterError`][kirin.exceptions.InterpreterError]:

```python
@dialect.register
class CfMethods(MethodTable):

    @impl(Assert)
    def assert_stmt(self, interp: Interpreter, frame: Frame, stmt: Assert):
        if frame.get(stmt.condition) is True:
            return ()

        if stmt.message:
            raise InterpreterError(frame.get(stmt.message))
        else:
            raise InterpreterError("assertion failed")
```

## Running the interpreter

To run the interpreter, you just need to pass the method to [`eval`][kirin.interp.Interpreter.eval]:

```python
from kirin.interp import Interpreter
from kirin.prelude import basic
interp = Interpreter(basic)

@basic
def main(a: int, b: int) -> int:
    return a + b

interp.eval(main, 1, 2)
```

## Overlaying

One of the most powerful features of the interpreter is overlaying. This allows you to override the implementation of a statement in a dialect by picking different order of method table lookup or even customize the method lookup. This is done by inheriting [`Interpreter`][kirin.interp.Interpreter] and define the class variable `keys`:

```python
class MyInterpreter(Interpreter):
    keys = ["my_overlay", "main"]
```

When using this new `MyInterpreter`, the method lookup will first look for the methods registered in the `my_overlay` key. If the method is not found, it will fall back to the `main` key. This allows you to override the implementation of a statement in a dialect without modifying the dialect itself.

We will talk about overlaying in abstract interpretation and analysis section which has more use cases.
