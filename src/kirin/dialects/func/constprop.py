from kirin import ir
from kirin.analysis.dataflow.constprop import (
    Const,
    ConstProp,
    ConstPropLattice,
    NotConst,
)
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Call, GetField, Lambda, Return
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
        # NOTE: support kwargs after Call stmt stores the key names
        n_total = len(values)
        if stmt.kwargs.data:
            kwargs = dict(
                zip(stmt.kwargs.data, values[n_total - len(stmt.kwargs.data) :])
            )
        else:
            kwargs = None

        # give up on dynamic method calls
        if not isinstance(values[0], Const):
            return ResultValue(NotConst())

        mt: ir.Method = values[0].data
        args = values[1 : n_total - len(stmt.kwargs.data)]
        args = interp.get_args(mt.arg_names[len(args) + 1 :], args, kwargs)
        if len(interp.state.frames) < interp.max_depth:
            return interp.eval(mt, args).to_result()
        return ResultValue(NotConst())

    @impl(Lambda)
    def lambda_(self, interp: ConstProp, stmt: Lambda, values: tuple):
        if all(isinstance(each, Const) for each in values):
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
