from kirin import ir
from kirin.decl import info, statement
from kirin.dialects.fcf.dialect import dialect


@statement(dialect=dialect)
class Foldl(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(ir.types.PyClass(ir.Method))
    coll: ir.SSAValue = info.argument(ir.types.Any)  # TODO: make this more precise
    init: ir.SSAValue = info.argument(ir.types.Any)
    result: ir.ResultValue = info.result(ir.types.Any)


@statement(dialect=dialect)
class Foldr(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(ir.types.PyClass(ir.Method))
    coll: ir.SSAValue = info.argument(ir.types.Any)
    init: ir.SSAValue = info.argument(ir.types.Any)
    result: ir.ResultValue = info.result(ir.types.Any)


@statement(dialect=dialect)
class Map(ir.Statement):
    """Apply a given kernel function to every item of an iterable.

    ## Example

    ```python
    from kirin.prelude import basic

    @basic(typeinfer=True)
    def myfunc(x: int):
        return x*3

    @basic(typeinfer=True)
    def main():
        return fcf.Map(fn=myfunc, coll=range(10))

    ```
    """

    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(ir.types.PyClass(ir.Method))
    """The kernel function to apply. The function should have signature `fn(x: int) -> Any`."""
    coll: ir.SSAValue = info.argument(ir.types.Any)
    """The iterable to map over."""
    result: ir.ResultValue = info.result(ir.types.List)
    """The list of results."""


@statement(dialect=dialect)
class Scan(ir.Statement):
    traits = frozenset({ir.FromPythonCall()})
    fn: ir.SSAValue = info.argument(ir.types.PyClass(ir.Method))
    init: ir.SSAValue = info.argument(ir.types.Any)
    coll: ir.SSAValue = info.argument(ir.types.List)
    result: ir.ResultValue = info.result(ir.types.Tuple[ir.types.Any, ir.types.List])
