import ast

from kirin import ir, types, lowering
from kirin.dialects import func
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("lowering.call")


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Call_local(
        self, state: lowering.LoweringState, callee: ir.SSAValue, node: ast.Call
    ) -> lowering.Result:
        args, keywords = self.__lower_Call_args_kwargs(state, node)
        stmt = func.Call(callee, args, kwargs=keywords)
        return lowering.Result(state.append_stmt(stmt))

    def lower_Call_global_method(
        self,
        state: lowering.LoweringState,
        method: ir.Method,
        node: ast.Call,
    ) -> lowering.Result:
        args, keywords = self.__lower_Call_args_kwargs(state, node)
        stmt = func.Invoke(args, callee=method, kwargs=keywords)
        stmt.result.type = method.return_type or types.Any
        return lowering.Result(state.append_stmt(stmt))

    def __lower_Call_args_kwargs(
        self,
        state: lowering.LoweringState,
        node: ast.Call,
    ):
        args: list[ir.SSAValue] = []
        for arg in node.args:
            if isinstance(arg, ast.Starred):  # TODO: support *args
                raise DialectLoweringError("starred arguments are not supported")
            else:
                args.append(state.visit(arg).expect_one())

        keywords = []
        for kw in node.keywords:
            keywords.append(kw.arg)
            args.append(state.visit(kw.value).expect_one())

        return tuple(args), tuple(keywords)
