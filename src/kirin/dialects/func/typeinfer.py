from __future__ import annotations

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
from kirin.dialects.func._dialect import dialect


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
    def call(self, interp_: TypeInference, frame: Frame, stmt: Call):
        # give up on dynamic method calls
        mt = interp_.maybe_const(stmt.callee, ir.Method)
        if mt is None:  # not a constant method
            return self._solve_method_type(interp_, frame, stmt)

        if mt.inferred:  # so we don't end up in infinite loop
            return (mt.return_type,)

        _, ret = interp_.call(
            mt.code,
            interp_.method_self(mt),
            *frame.get_values(stmt.inputs),
            **{k: v for k, v in zip(stmt.keys, frame.get_values(stmt.kwargs))},
        )
        return (ret,)

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
        # NOTE: we are not using [...] below to be compatible with 3.10
        resolve.solve(args, types.Tuple.where(frame.get_values(stmt.inputs)))
        return (resolve.substitute(result),)

    @impl(Invoke)
    def invoke(self, interp_: TypeInference, frame: Frame, stmt: Invoke):
        if stmt.callee.inferred:  # so we don't end up in infinite loop
            return (stmt.callee.return_type,)

        _, ret = interp_.call(
            stmt.callee.code,
            interp_.method_self(stmt.callee),
            *frame.get_values(stmt.inputs),
        )
        return (ret,)

    @impl(Lambda)
    def lambda_(
        self, interp_: TypeInference, frame: Frame[types.TypeAttribute], stmt: Lambda
    ):
        body_frame, ret = interp_.call(
            stmt,
            types.MethodType,
            *tuple(arg.type for arg in stmt.body.blocks[0].args[1:]),
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
