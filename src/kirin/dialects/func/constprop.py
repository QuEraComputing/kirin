from typing import Iterable

from kirin import ir
from kirin.analysis.dataflow.constprop import (
    Const,
    ConstProp,
    ConstPropBottom,
    ConstPropLattice,
    NotConst,
    PartialLambda,
)
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Call, GetField, Invoke, Lambda, Return
from kirin.interp import DialectInterpreter, ResultValue, ReturnValue, impl


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(Return)
    def return_(self, interp: ConstProp, stmt: Return, values: tuple) -> ReturnValue:
        if not values:
            return ReturnValue(Const(None))
        else:
            return ReturnValue(*values)

    @impl(Call)
    def call(self, interp: ConstProp, stmt: Call, values: tuple[ConstPropLattice, ...]):
        # give up on dynamic method calls
        if not values:  # err
            return ResultValue(ConstPropBottom())

        if isinstance(values[0], PartialLambda):
            return ResultValue(
                self._call_lambda(
                    interp,
                    values[0],
                    interp.permute_values(values[0].argnames, values[1:], stmt.kwargs),
                )
            )

        if not isinstance(values[0], Const):
            return ResultValue(NotConst())

        mt: ir.Method = values[0].data
        return ResultValue(
            self._invoke_method(
                interp,
                mt,
                interp.permute_values(mt.arg_names, values[1:], stmt.kwargs),
                stmt.results,
            )
        )

    def _call_lambda(
        self,
        interp: ConstProp,
        callee: PartialLambda,
        args: tuple[ConstPropLattice, ...],
    ):
        # NOTE: we still use PartialLambda because
        # we want to gurantee what we receive here in captured
        # values are all lattice elements and not just obtain via
        # Const(Method(...)) which is Any.
        mt = ir.Method(
            mod=None,
            py_func=None,
            sym_name=None,
            arg_names=callee.argnames,
            dialects=interp.dialects,
            code=callee.code,
            fields=callee.captured,
        )
        return interp.eval(mt, args).expect()

    @impl(Invoke)
    def invoke(
        self, interp: ConstProp, stmt: Invoke, values: tuple[ConstPropLattice, ...]
    ):
        return ResultValue(
            self._invoke_method(
                interp,
                stmt.callee,
                interp.permute_values(stmt.callee.arg_names, values, stmt.kwargs),
                stmt.results,
            )
        )

    def _invoke_method(
        self,
        interp: ConstProp,
        mt: ir.Method,
        values: tuple[ConstPropLattice, ...],
        results: Iterable[ir.ResultValue],
    ):
        if len(interp.state.frames) < interp.max_depth:
            return interp.eval(mt, values).expect()
        return interp.bottom

    @impl(Lambda)
    def lambda_(self, interp: ConstProp, stmt: Lambda, values: tuple):
        arg_names = [
            arg.name or str(idx) for idx, arg in enumerate(stmt.body.blocks[0].args)
        ]
        if not stmt.body.blocks.isempty() and all(
            isinstance(each, Const) for each in values
        ):
            return ResultValue(
                Const(
                    ir.Method(
                        mod=None,
                        py_func=None,
                        sym_name=stmt.sym_name,
                        arg_names=arg_names,
                        dialects=interp.dialects,
                        code=stmt,
                        fields=tuple(each.data for each in values),
                    )
                )
            )

        return ResultValue(
            PartialLambda(
                arg_names,
                stmt,
                values,
            )
        )

    @impl(GetField)
    def getfield(self, interp: ConstProp, stmt: GetField, values: tuple):
        callee_self = values[0]
        if isinstance(callee_self, Const):
            mt: ir.Method = callee_self.data
            return ResultValue(Const(mt.fields[stmt.field]))
        elif isinstance(callee_self, PartialLambda):
            return ResultValue(callee_self.captured[stmt.field])
        return ResultValue(NotConst())
