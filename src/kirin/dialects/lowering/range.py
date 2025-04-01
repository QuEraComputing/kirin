import ast

from kirin import ir, lowering2
from kirin.dialects.py.range import Range as PyRange
from kirin.dialects.ilist.stmts import Range as IListRange

ilist = ir.Dialect("lowering.range.ilist")
"""provides the syntax sugar from built-in range() function to ilist.range()
"""
py = ir.Dialect("lowering.range.py")
"""provides the syntax sugar from built-in range() function to py.range()
"""


@py.register
class PyLowering(lowering2.FromPythonAST):

    def lower_Call_range(
        self, state: lowering2.State, node: ast.Call
    ) -> lowering2.Result:
        return lowering2.FromPythonRangeLike().lower(PyRange, state, node)


@ilist.register
class IListLowering(lowering2.FromPythonAST):

    def lower_Call_range(
        self, state: lowering2.State, node: ast.Call
    ) -> lowering2.Result:
        return lowering2.FromPythonRangeLike().lower(IListRange, state, node)
