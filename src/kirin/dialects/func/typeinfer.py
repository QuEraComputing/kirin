from typing import Iterable

from kirin import ir
from kirin.interp import Frame, MethodTable, ReturnValue, impl
from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.func.stmts import (
    Call,
    Invoke,
    Lambda,
    Return,
    GetField,
    ConstantNone,
)
from kirin.dialects.func.dialect import dialect


# NOTE: a lot of the type infer rules are same as the builtin dialect
@dialect.register(key="typeinfer")
class TypeInfer(MethodTable):

    @impl(ConstantNone)
    def const_none(self, interp: TypeInference, frame: Frame, stmt: ConstantNone):
        return (ir.types.NoneType,)

    @impl(Return)
    def return_(self, interp: TypeInference, frame: Frame, stmt: Return) -> ReturnValue:
        return ReturnValue(frame.get(stmt.value))

    @impl(Call)
    def call(self, interp: TypeInference, frame: Frame, stmt: Call):
        # give up on dynamic method calls
        callee = frame.get(stmt.callee)
        if not isinstance(callee, ir.types.Const):
            return (stmt.result.type,)

        mt: ir.Method = callee.data
        return self._invoke_method(
            interp,
            mt,
            stmt.args[1:],
            interp.permute_values(
                mt.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
            ),
        )

    @impl(Invoke)
    def invoke(self, interp: TypeInference, frame: Frame, stmt: Invoke):
        return self._invoke_method(
            interp,
            stmt.callee,
            stmt.inputs,
            interp.permute_values(
                stmt.callee.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
            ),
        )

    def _invoke_method(
        self,
        interp: TypeInference,
        mt: ir.Method,
        args: Iterable[ir.SSAValue],
        values: tuple,
    ):
        if mt.inferred:  # so we don't end up in infinite loop
            return (mt.return_type,)

        # NOTE: narrowing the argument type based on method signature
        inputs = tuple(
            typ.meet(input_typ) for typ, input_typ in zip(mt.arg_types, values)
        )

        # NOTE: we use lower bound here because function call contains an
        # implicit type check at call site. This will be validated either compile time
        # or runtime.
        # update the results with the narrowed types
        for arg, typ in zip(args, inputs):
            interp.results[arg] = typ

        return interp.eval(mt, inputs).to_result()

    @impl(Lambda)
    def lambda_(self, interp: TypeInference, frame, stmt: Lambda):
        return (ir.types.PyClass(ir.Method),)

    @impl(GetField)
    def getfield(self, interp: TypeInference, frame, stmt: GetField):
        return (stmt.result.type,)
