"""Attribute access dialect for Python.

This module contains the dialect for the Python attribute access statement, including:

- The `GetAttr` statement class.
- The lowering pass for the attribute access statement.
- The concrete implementation of the attribute access statement.

This dialect maps `ast.Attribute` nodes to the `GetAttr` statement.
"""

import ast

from kirin import ir, interp, lowering2
from kirin.decl import info, statement

dialect = ir.Dialect("py.attr")


@statement(dialect=dialect)
class GetAttr(ir.Statement):
    name = "getattr"
    traits = frozenset({lowering2.FromPythonCall()})
    obj: ir.SSAValue = info.argument(print=False)
    attrname: str = info.attribute()
    result: ir.ResultValue = info.result()


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(GetAttr)
    def getattr(self, interp: interp.Interpreter, frame: interp.Frame, stmt: GetAttr):
        return (getattr(frame.get(stmt.obj), stmt.attrname),)


@dialect.register
class Lowering(lowering2.FromPythonAST):

    def lower_Attribute(
        self, state: lowering2.State, node: ast.Attribute
    ) -> lowering2.Result:
        from kirin.dialects.py import Constant

        if not isinstance(node.ctx, ast.Load):
            raise lowering2.DialectLoweringError(
                f"unsupported attribute context {node.ctx}"
            )

        # NOTE: eagerly load global variables
        value = state.get_global(node, no_raise=True)
        if value is not None:
            return state.current_frame.push(Constant(value.data))

        value = state.lower(node.value).expect_one()
        return state.current_frame.push(GetAttr(obj=value, attrname=node.attr))
