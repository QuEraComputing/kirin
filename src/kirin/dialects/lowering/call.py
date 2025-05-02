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
        args, kwargs, keys = self.__lower_Call_args_kwargs(state, node)
        stmt = func.Call(callee, args, kwargs, keys=keys)
        stmt.source = source
        return state.current_frame.push(stmt)

    def lower_Call_global_method(
        self,
        state: lowering.State,
        method: ir.Method,
        node: ast.Call,
    ) -> lowering.Result:
        source = state.source
        args, kwargs, keys = self.__lower_Call_args_kwargs(state, node)
        inputs: list[ir.SSAValue] = [*args]
        kwargs_ = {k: v for k, v in zip(keys, kwargs)}

        if method.arg_names is None and keys:
            raise lowering.BuildError("method has no argument names, cannot use kwargs")

        if method.arg_names and keys:
            for name in method.arg_names:
                if name in keys:
                    inputs.append(kwargs_[name])

        stmt = func.Invoke(tuple(inputs), callee=method)
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

        keys: list[str] = []
        kwargs: list[ir.SSAValue] = []
        for kw in node.keywords:
            if kw.arg is None:
                raise lowering.BuildError("keyword argument must have a name")
            keys.append(kw.arg)
            kwargs.append(state.lower(kw.value).expect_one())

        return tuple(args), tuple(kwargs), tuple(keys)
