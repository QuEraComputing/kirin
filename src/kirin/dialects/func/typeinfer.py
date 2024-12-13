from typing import Iterable

from kirin import ir
from kirin.interp import ReturnValue, DialectMethodTable, impl
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
class TypeInfer(DialectMethodTable):

    @impl(ConstantNone)
    def const_none(self, interp: TypeInference, stmt: ConstantNone, values: tuple[()]):
        return (ir.types.NoneType,)

    @impl(Return)
    def return_(
        self, interp: TypeInference, stmt: Return, values: tuple
    ) -> ReturnValue:
        return ReturnValue(*values)

    @impl(Call)
    def call(self, interp: TypeInference, stmt: Call, values: tuple):
        # give up on dynamic method calls
        if not isinstance(values[0], ir.types.Const):
            return (stmt.result.type,)

        mt: ir.Method = values[0].data
        return self._invoke_method(
            interp,
            mt,
            stmt.args[1:],
            interp.permute_values(mt.arg_names, values[1:], stmt.kwargs),
        )

    @impl(Invoke)
    def invoke(self, interp: TypeInference, stmt: Invoke, values: tuple):
        return self._invoke_method(
            interp,
            stmt.callee,
            stmt.args[1:],
            interp.permute_values(stmt.callee.arg_names, values, stmt.kwargs),
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
    def lambda_(self, interp: TypeInference, stmt: Lambda, values: tuple):
        return (ir.types.PyClass(ir.Method),)

    @impl(GetField)
    def getfield(self, interp: TypeInference, stmt: GetField, values: tuple):
        return (stmt.result.type,)
