import ast

from kirin import ir, types, lowering
from kirin.dialects import func

dialect = ir.Dialect("lowering.call")


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Call_local(
        self, state: lowering.State, callee: ir.SSAValue, node: ast.Call
    ) -> lowering.Result:
        source = state.source
        args, keywords = self.__lower_Call_args_kwargs(state, node)
        stmt = func.Call(callee, args, kwargs=keywords)
        stmt.source = source
        return state.current_frame.push(stmt)

    def lower_Call_global_method(
        self,
        state: lowering.State,
        method: ir.Method,
        node: ast.Call,
    ) -> lowering.Result:
        source = state.source
        args, keywords = self.__lower_Call_args_kwargs(state, node)
        stmt = func.Invoke(args, callee=method, kwargs=keywords)
        stmt.result.type = method.return_type or types.Any
        stmt.source = source
        return state.current_frame.push(stmt)

    def __lower_Call_args_kwargs(
        self,
        state: lowering.State,
        node: ast.Call,
    ):
        args: list[ir.SSAValue] = []
        for arg in node.args:
            if isinstance(arg, ast.Starred):  # TODO: support *args
                raise lowering.BuildError("starred arguments are not supported")
            else:
                args.append(state.lower(arg).expect_one())

        keywords = []
        for kw in node.keywords:
            keywords.append(kw.arg)
            args.append(state.lower(kw.value).expect_one())

        return tuple(args), tuple(keywords)
