from typing import Iterable

from kirin import ir
from kirin.interp import Result, ReturnValue, DialectInterpreter, impl
from kirin.analysis import const
from kirin.dialects.func.stmts import Call, Invoke, Lambda, Return, GetField
from kirin.dialects.func.dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(Return)
    def return_(
        self, interp: const.Propagate, stmt: Return, values: tuple[const.JointResult]
    ) -> Result[const.JointResult]:
        if not values:
            return ReturnValue(
                const.JointResult(const.Value(None), const.PurityBottom())
            )
        else:
            return ReturnValue(*values)

    @impl(Call)
    def call(
        self, interp: const.Propagate, stmt: Call, values: tuple[const.JointResult, ...]
    ) -> Result[const.JointResult]:
        # give up on dynamic method calls
        if not values:  # err
            return (const.JointResult.bottom(),)

        if isinstance(callee := values[0].const, const.PartialLambda):
            return (
                self._call_lambda(
                    interp,
                    callee,
                    interp.permute_values(callee.argnames, values[1:], stmt.kwargs),
                ),
            )

        if not isinstance(callee := values[0].const, const.Value):
            return (const.JointResult.bottom(),)

        mt: ir.Method = callee.data
        return (
            self._invoke_method(
                interp,
                mt,
                interp.permute_values(mt.arg_names, values[1:], stmt.kwargs),
                stmt.results,
            ),
        )

    def _call_lambda(
        self,
        interp: const.Propagate,
        callee: const.PartialLambda,
        args: tuple[const.JointResult, ...],
    ):
        # NOTE: we still use PartialLambda because
        # we want to gurantee what we receive here in captured
        # values are all lattice elements and not just obtain via
        # Const(Method(...)) which is Any.
        if (trait := callee.code.get_trait(ir.SymbolOpInterface)) is not None:
            name = trait.get_sym_name(callee.code).data
        else:
            name = "lambda"

        mt = ir.Method(
            mod=None,
            py_func=None,
            sym_name=name,
            arg_names=callee.argnames,
            dialects=interp.dialects,
            code=callee.code,
            fields=callee.captured,
        )
        return interp.eval(mt, args).expect()

    @impl(Invoke)
    def invoke(
        self,
        interp: const.Propagate,
        stmt: Invoke,
        values: tuple[const.JointResult, ...],
    ) -> Result[const.JointResult]:
        return (
            self._invoke_method(
                interp,
                stmt.callee,
                interp.permute_values(stmt.callee.arg_names, values, stmt.kwargs),
                stmt.results,
            ),
        )

    def _invoke_method(
        self,
        interp: const.Propagate,
        mt: ir.Method,
        values: tuple[const.JointResult, ...],
        results: Iterable[ir.ResultValue],
    ):
        return interp.eval(mt, values).expect()

    @impl(Lambda)
    def lambda_(
        self, interp: const.Propagate, stmt: Lambda, values: tuple
    ) -> Result[const.JointResult]:
        arg_names = [
            arg.name or str(idx) for idx, arg in enumerate(stmt.body.blocks[0].args)
        ]
        if not stmt.body.blocks.isempty() and all(
            isinstance(each, const.Value) for each in values
        ):
            return (
                const.JointResult(
                    const.Value(
                        ir.Method(
                            mod=None,
                            py_func=None,
                            sym_name=stmt.sym_name,
                            arg_names=arg_names,
                            dialects=interp.dialects,
                            code=stmt,
                            fields=tuple(each.data for each in values),
                        )
                    ),
                    const.Pure(),
                ),
            )

        return (
            const.JointResult(
                const.PartialLambda(
                    arg_names,
                    stmt,
                    values,
                ),
                const.Pure(),
            ),
        )

    @impl(GetField)
    def getfield(
        self, interp: const.Propagate, stmt: GetField, values: tuple
    ) -> Result[const.JointResult]:
        callee_self = values[0]
        if isinstance(callee_self, const.Value):
            mt: ir.Method = callee_self.data
            return (
                const.JointResult(const.Value(mt.fields[stmt.field]), const.Pure()),
            )
        elif isinstance(callee_self, const.PartialLambda):
            return (const.JointResult(callee_self.captured[stmt.field], const.Pure()),)
        return (const.JointResult(const.Unknown(), const.Pure()),)
