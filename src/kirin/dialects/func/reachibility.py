from kirin.analysis.dataflow.reachable import ReachableAnalysis
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Lambda
from kirin.dialects.func.typeinfer import TypeInfer
from kirin.interp import Result, ReturnValue, impl


# NOTE: a lot of the type infer rules are same as the builtin dialect
@dialect.register(key="reachibility")
class ReachibilityInfer(TypeInfer):

    # NOTE: this allows forward dataflow walk into the lambda body
    @impl(Lambda)
    def lambda_(self, interp: ReachableAnalysis, stmt: Lambda, values: tuple) -> Result:
        # NOTE: skip the lambda frame here, because we want to have it as part of CFG
        result = interp.run_ssacfg_region(
            stmt.body, (interp.bottom, *tuple(interp.bottom for _ in stmt.args))
        ).to_result()
        if isinstance(result, ReturnValue):
            stmt.signature.output = result.result
        return result
