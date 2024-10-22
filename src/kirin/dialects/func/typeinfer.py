from kirin import ir
from kirin.analysis.dataflow.typeinfer import TypeInference
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Call, ConstantMethod, GetField, Lambda, Return
from kirin.dialects.py import types
from kirin.interp import DialectInterpreter, ResultValue, ReturnValue, impl


# NOTE: a lot of the type infer rules are same as the builtin dialect
@dialect.register(key="typeinfer")
class TypeInfer(DialectInterpreter):

    @impl(ConstantMethod)
    def constant(
        self, interp: TypeInference, stmt: ConstantMethod, values: tuple
    ) -> ResultValue:
        return ResultValue(types.PyConst(stmt.value))

    @impl(Return)
    def return_(
        self, interp: TypeInference, stmt: Return, values: tuple
    ) -> ReturnValue:
        if not values:
            return ReturnValue(types.NoneType)
        else:
            return ReturnValue(*values)

    @impl(Call)
    def call(self, interp: TypeInference, stmt: Call, values: tuple):
        # NOTE: support kwargs after Call stmt stores the key names
        n_total = len(values)
        if stmt.kwargs.data:
            kwargs = dict(
                zip(stmt.kwargs.data, values[n_total - len(stmt.kwargs.data) :])
            )
        else:
            kwargs = None

        # give up on dynamic method calls
        if not isinstance(values[0], types.PyConst):
            return ResultValue(stmt.result.type)

        mt: ir.Method = values[0].data
        if mt.inferred:  # so we don't end up in infinite loop
            return ResultValue(mt.return_type)

        args = values[1 : n_total - len(stmt.kwargs.data)]
        args = interp.get_args(mt.arg_names[len(args) + 1 :], args, kwargs)
        narrow_arg_types = tuple(
            typ.meet(input_typ) for typ, input_typ in zip(mt.arg_types, args)
        )
        # NOTE: we use lower bound here because function call contains an
        # implicit type check at call site. This will be validated either compile time
        # or runtime.
        # update the results with the narrowed types
        for arg, typ in zip(stmt.args[1:], narrow_arg_types):
            interp.results[arg] = typ

        if len(interp.state.frames) < interp.max_depth:
            return interp.eval(mt, narrow_arg_types).to_result()
        # max depth reached, error
        return ResultValue(types.Bottom)

    @impl(Lambda)
    def lambda_(self, interp: TypeInference, stmt: Lambda, values: tuple):
        return ResultValue(types.PyClass(ir.Method))

    @impl(GetField)
    def getfield(self, interp: TypeInference, stmt: GetField, values: tuple):
        return ResultValue(stmt.result.type)
