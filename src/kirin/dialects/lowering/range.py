import ast

from kirin import ir, lowering
from kirin.dialects.py.range import Range as PyRange
from kirin.dialects.ilist.stmts import Range as IListRange

ilist = ir.Dialect("lowering.range.ilist")
"""provides the syntax sugar from built-in range() function to ilist.range()
"""
py = ir.Dialect("lowering.range.py")
"""provides the syntax sugar from built-in range() function to py.range()
"""


@py.register
class PyLowering(lowering.FromPythonAST):

    def lower_Call_range(
        self, state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return ir.FromPythonRangeLike().lower(PyRange, state, node)


@ilist.register
class IListLowering(lowering.FromPythonAST):

    def lower_Call_range(
        self, state: lowering.LoweringState, node: ast.Call
    ) -> lowering.Result:
        return ir.FromPythonRangeLike().lower(IListRange, state, node)
