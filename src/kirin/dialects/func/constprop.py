from typing import Iterable

from kirin import ir
from kirin.analysis import const
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Call, GetField, Invoke, Lambda, Return
from kirin.interp import DialectInterpreter, ResultValue, ReturnValue, impl


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(Return)
    def return_(
        self, interp: const.Propagate, stmt: Return, values: tuple
    ) -> ReturnValue:
        if not values:
            return ReturnValue(const.Value(None))
        else:
            return ReturnValue(*values)

    @impl(Call)
    def call(
        self, interp: const.Propagate, stmt: Call, values: tuple[const.Result, ...]
    ):
        # give up on dynamic method calls
        if not values:  # err
            return ResultValue(const.Bottom())

        if isinstance(values[0], const.PartialLambda):
            return ResultValue(
                self._call_lambda(
                    interp,
                    values[0],
                    interp.permute_values(values[0].argnames, values[1:], stmt.kwargs),
                )
            )

        if not isinstance(values[0], const.Value):
            return ResultValue(const.Unknown())

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
        interp: const.Propagate,
        callee: const.PartialLambda,
        args: tuple[const.Result, ...],
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
        self, interp: const.Propagate, stmt: Invoke, values: tuple[const.Result, ...]
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
        interp: const.Propagate,
        mt: ir.Method,
        values: tuple[const.Result, ...],
        results: Iterable[ir.ResultValue],
    ):
        if len(interp.state.frames) < interp.max_depth:
            return interp.eval(mt, values).expect()
        return interp.bottom

    @impl(Lambda)
    def lambda_(self, interp: const.Propagate, stmt: Lambda, values: tuple):
        arg_names = [
            arg.name or str(idx) for idx, arg in enumerate(stmt.body.blocks[0].args)
        ]
        if not stmt.body.blocks.isempty() and all(
            isinstance(each, const.Value) for each in values
        ):
            return ResultValue(
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
                )
            )

        return ResultValue(
            const.PartialLambda(
                arg_names,
                stmt,
                values,
            )
        )

    @impl(GetField)
    def getfield(self, interp: const.Propagate, stmt: GetField, values: tuple):
        callee_self = values[0]
        if isinstance(callee_self, const.Value):
            mt: ir.Method = callee_self.data
            return ResultValue(const.Value(mt.fields[stmt.field]))
        elif isinstance(callee_self, const.PartialLambda):
            return ResultValue(callee_self.captured[stmt.field])
        return ResultValue(const.Unknown())
