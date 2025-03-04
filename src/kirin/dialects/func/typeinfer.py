from typing import Iterable, cast

from kirin import ir, types
from kirin.interp import Frame, MethodTable, ReturnValue, impl
from kirin.analysis import const
from kirin.analysis.typeinfer import TypeInference, TypeResolution
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
        return (types.NoneType,)

    @impl(Return)
    def return_(
        self, interp: TypeInference, frame: Frame[types.TypeAttribute], stmt: Return
    ) -> ReturnValue:
        if (
            isinstance(hint := stmt.value.hints.get("const"), const.Value)
            and hint.data is not None
        ):
            return ReturnValue(types.Literal(hint.data, frame.get(stmt.value)))
        return ReturnValue(frame.get(stmt.value))

    @impl(Call)
    def call(self, interp: TypeInference, frame: Frame, stmt: Call):
        # give up on dynamic method calls
        mt = interp.maybe_const(stmt.callee, ir.Method)
        if mt is None:
            return self._solve_method_type(interp, frame, stmt)
        return self._invoke_method(
            interp,
            frame,
            mt,
            stmt.args[1:],
            interp.permute_values(
                mt.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
            ),
        )

    def _solve_method_type(self, interp: TypeInference, frame: Frame, stmt: Call):
        mt_inferred = frame.get(stmt.callee)
        if not isinstance(mt_inferred, types.Generic):
            return (types.Bottom,)

        if len(mt_inferred.vars) != 2:
            return (types.Bottom,)

        args = mt_inferred.vars[0]
        result = mt_inferred.vars[1]
        if not args.is_subseteq(types.Tuple):
            return (types.Bottom,)

        resolve = TypeResolution()
        args = cast(types.Generic, args)
        for arg, value in zip(args.vars, frame.get_values(stmt.inputs)):
            resolve.solve(arg, value)
        return (resolve.substitute(result),)

    @impl(Invoke)
    def invoke(self, interp: TypeInference, frame: Frame, stmt: Invoke):
        return self._invoke_method(
            interp,
            frame,
            stmt.callee,
            stmt.inputs,
            interp.permute_values(
                stmt.callee.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
            ),
        )

    def _invoke_method(
        self,
        interp: TypeInference,
        frame: Frame,
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
        frame.set_values(args, inputs)
        _, ret = interp.run_method(mt, inputs)
        return (ret,)

    @impl(Lambda)
    def lambda_(
        self, interp_: TypeInference, frame: Frame[types.TypeAttribute], stmt: Lambda
    ):
        body_frame, ret = interp_.run_callable(
            stmt,
            (types.MethodType,)
            + tuple(arg.type for arg in stmt.body.blocks[0].args[1:]),
        )
        argtypes = tuple(arg.type for arg in stmt.body.blocks[0].args[1:])
        ret = types.MethodType[[*argtypes], ret]
        frame.entries.update(body_frame.entries)  # pass results back to upper frame
        self_ = stmt.body.blocks[0].args[0]
        frame.set(self_, ret)
        return (ret,)

    @impl(GetField)
    def getfield(self, interp: TypeInference, frame, stmt: GetField):
        return (stmt.result.type,)
