import ast

from kirin import ir, types, lowering2
from kirin.dialects import func
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("lowering.call")


@dialect.register
class Lowering(lowering2.FromPythonAST):

    def lower_Call_local(
        self, state: lowering2.State, callee: ir.SSAValue, node: ast.Call
    ) -> lowering2.Result:
        args, keywords = self.__lower_Call_args_kwargs(state, node)
        stmt = func.Call(callee, args, kwargs=keywords)
        return state.current_frame.push(stmt)

    def lower_Call_global_method(
        self,
        state: lowering2.State,
        method: ir.Method,
        node: ast.Call,
    ) -> lowering2.Result:
        args, keywords = self.__lower_Call_args_kwargs(state, node)
        stmt = func.Invoke(args, callee=method, kwargs=keywords)
        stmt.result.type = method.return_type or types.Any
        return state.current_frame.push(stmt)

    def __lower_Call_args_kwargs(
        self,
        state: lowering2.State,
        node: ast.Call,
    ):
        args: list[ir.SSAValue] = []
        for arg in node.args:
            if isinstance(arg, ast.Starred):  # TODO: support *args
                raise DialectLoweringError("starred arguments are not supported")
            else:
                args.append(state.lower(arg).expect_one())

        keywords = []
        for kw in node.keywords:
            keywords.append(kw.arg)
            args.append(state.lower(kw.value).expect_one())

        return tuple(args), tuple(keywords)
