from kirin.analysis.dataflow.typeinfer import TypeInference
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.interp import Interpreter
from kirin.dialects.func.stmts import Call, Return
from kirin.dialects.py import types
from kirin.interp import ResultValue, ReturnValue, impl


# NOTE: a lot of the type infer rules are same as the builtin dialect
@dialect.register(key="typeinfer")
class TypeInfer(Interpreter):
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
            return ResultValue(types.Any)

        return interp.eval(
            values[0].data, values[1 : n_total - len(stmt.kwargs.data)], kwargs
        ).to_result()
