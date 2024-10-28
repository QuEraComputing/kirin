from typing import Iterable

from kirin import ir
from kirin.analysis.dataflow.constprop import (
    Const,
    ConstProp,
    ConstPropLattice,
    NotConst,
    NotPure,
)
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Call, GetField, Invoke, Lambda, Return
from kirin.interp import DialectInterpreter, ResultValue, ReturnValue, impl


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(Return)
    def return_(self, interp: ConstProp, stmt: Return, values: tuple) -> ReturnValue:
        frame = interp.state.current_frame()
        # we executed a non-pure statement, cannot return const
        if frame.extra is not None and frame.extra.not_pure:
            return ReturnValue(NotPure())

        if not values:
            return ReturnValue(Const(None))
        else:
            return ReturnValue(*values)

    @impl(Call)
    def call(self, interp: ConstProp, stmt: Call, values: tuple[ConstPropLattice, ...]):
        # give up on dynamic method calls
        if not isinstance(values[0], Const):
            return ResultValue(NotConst())

        mt: ir.Method = values[0].data
        return self._invoke_method(
            interp,
            mt,
            interp.permute_values(mt, values[1:], stmt.kwargs),
            stmt.results,
        )

    @impl(Invoke)
    def invoke(
        self, interp: ConstProp, stmt: Invoke, values: tuple[ConstPropLattice, ...]
    ):
        return self._invoke_method(
            interp,
            stmt.callee,
            interp.permute_values(stmt.callee, values, stmt.kwargs),
            stmt.results,
        )

    def _invoke_method(
        self,
        interp: ConstProp,
        mt: ir.Method,
        values: tuple[ConstPropLattice, ...],
        results: Iterable[ir.ResultValue],
    ):
        if len(interp.state.frames) < interp.max_depth:
            result = interp.eval(mt, values).expect()
            if isinstance(result, NotPure):
                frame = interp.state.current_frame()
                for _result in results:
                    frame.entries[_result] = NotPure()
                return ReturnValue(result)
            else:
                return ResultValue(result)
        return ResultValue(interp.bottom)

    @impl(Lambda)
    def lambda_(self, interp: ConstProp, stmt: Lambda, values: tuple):
        if not stmt.body.blocks.isempty() and all(
            isinstance(each, Const) for each in values
        ):
            return ResultValue(
                Const(
                    ir.Method(
                        mod=None,
                        py_func=None,
                        sym_name=stmt.name,
                        arg_names=[
                            arg.name or str(idx)
                            for idx, arg in enumerate(stmt.body.blocks[0].args)
                        ],
                        dialects=interp.dialects,
                        code=stmt,
                        fields=tuple(each.data for each in values),
                    )
                )
            )
        return ResultValue(NotConst())

    @impl(GetField)
    def getfield(self, interp: ConstProp, stmt: GetField, values: tuple):
        if isinstance(values[0], Const):
            mt: ir.Method = values[0].data
            return ResultValue(Const(mt.fields[stmt.field]))
        return ResultValue(NotConst())
