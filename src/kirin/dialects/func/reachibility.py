from kirin import ir
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Lambda
from kirin.dialects.func.typeinfer import TypeInfer
from kirin.interp import AbstractInterpreter, Result, ReturnValue, impl


# NOTE: a lot of the type infer rules are same as the builtin dialect
@dialect.register(key="reachibility")
class ReachibilityInfer(TypeInfer):

    # NOTE: this allows forward dataflow walk into the lambda body
    @impl(Lambda)
    def lambda_(
        self, interp: AbstractInterpreter, stmt: Lambda, values: tuple
    ) -> Result:
        mt = ir.Method(
            mod=None,
            py_func=None,
            sym_name=stmt.sym_name,
            arg_names=[f.name or "" for f in stmt.args],
            dialects=interp.dialects,
            code=stmt,
        )

        result = interp.eval(
            mt, (interp.bottom, *tuple(f.type for f in stmt.args))
        ).to_result()

        if isinstance(result, ReturnValue):
            stmt.signature.output = result.result
        return result
