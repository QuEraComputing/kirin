"""The range dialect for Python.

This dialect models the builtin `range()` function in Python.

The dialect includes:
- The `Range` statement class.
- The lowering pass for the `range()` function.

This dialect does not include a concrete implementation or type inference
for the `range()` function. One needs to use other dialect for the concrete
implementation and type inference, e.g., `ilist` dialect.
"""

from kirin import ir, types, interp
from kirin.decl import info, statement
from kirin.dialects import eltype

dialect = ir.Dialect("py.range")


@statement(dialect=dialect)
class Range(ir.Statement):
    name = "range"
    traits = frozenset({ir.Pure(), ir.FromPythonRangeLike()})
    start: ir.SSAValue = info.argument(types.Int)
    stop: ir.SSAValue = info.argument(types.Int)
    step: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(types.PyClass(range))


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(eltype.ElType, types.PyClass(range))
    def eltype_range(self, interp_, frame: interp.Frame, stmt: eltype.ElType):
        return (types.Int,)
