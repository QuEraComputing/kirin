"""Boolean operators for Python dialect.

This module contains the dialect for the Python boolean operators, including:

- The `And` and `Or` statement classes.
- The lowering pass for the boolean operators.
- The concrete implementation of the boolean operators.
- The Julia emitter for the boolean operators.

This dialect maps `ast.BoolOp` nodes to the `And` and `Or` statements.
"""

import ast

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.emit.julia import EmitJulia, EmitStrFrame
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("py.boolop")


@statement
class BoolOp(ir.Statement):
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    lhs: ir.SSAValue = info.argument(print=False)
    rhs: ir.SSAValue = info.argument(print=False)
    result: ir.ResultValue = info.result(types.Bool)


@statement(dialect=dialect)
class And(BoolOp):
    name = "and"


@statement(dialect=dialect)
class Or(BoolOp):
    name = "or"


@dialect.register
class PythonLowering(lowering.FromPythonAST):

    def lower_BoolOp(
        self, state: lowering.LoweringState, node: ast.BoolOp
    ) -> lowering.Result:
        lhs = state.visit(node.values[0]).expect_one()
        match node.op:
            case ast.And():
                boolop = And
            case ast.Or():
                boolop = Or
            case _:
                raise DialectLoweringError(f"unsupported boolop {node.op}")

        for value in node.values[1:]:
            lhs = state.append_stmt(
                boolop(lhs=lhs, rhs=state.visit(value).expect_one())
            ).result
        return lowering.Result(lhs)


@dialect.register
class BoolOpMethod(interp.MethodTable):

    @interp.impl(And)
    def and_(self, interp, frame: interp.Frame, stmt: And):
        return (frame.get(stmt.lhs) and frame.get(stmt.rhs),)

    @interp.impl(Or)
    def or_(self, interp, frame: interp.Frame, stmt: Or):
        return (frame.get(stmt.lhs) or frame.get(stmt.rhs),)


@dialect.register(key="emit.julia")
class JuliaTable(interp.MethodTable):

    @interp.impl(And)
    def emit_And(self, emit: EmitJulia, frame: EmitStrFrame, stmt: And):
        return emit.emit_binaryop(frame, "&&", stmt.lhs, stmt.rhs, stmt.result)

    @interp.impl(Or)
    def emit_Or(self, emit: EmitJulia, frame: EmitStrFrame, stmt: Or):
        return emit.emit_binaryop(frame, "||", stmt.lhs, stmt.rhs, stmt.result)
