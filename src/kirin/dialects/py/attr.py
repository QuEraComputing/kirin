import ast

from kirin import ir, interp, lowering, exceptions
from kirin.decl import info, statement

dialect = ir.Dialect("py.attr")


@statement(dialect=dialect)
class GetAttr(ir.Statement):
    name = "getattr"
    traits = frozenset({ir.FromPythonCall()})
    obj: ir.SSAValue = info.argument(print=False)
    attrname: str = info.attribute(property=True)
    result: ir.ResultValue = info.result()


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(GetAttr)
    def getattr(self, interp: interp.Interpreter, frame: interp.Frame, stmt: GetAttr):
        return getattr(frame.get(stmt.obj), stmt.attrname)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Attribute(
        self, state: lowering.LoweringState, node: ast.Attribute
    ) -> lowering.Result:
        if not isinstance(node.ctx, ast.Load):
            raise exceptions.DialectLoweringError(
                f"unsupported attribute context {node.ctx}"
            )
        value = state.visit(node.value).expect_one()
        stmt = GetAttr(obj=value, attrname=node.attr)
        state.append_stmt(stmt)
        return lowering.Result(stmt)
