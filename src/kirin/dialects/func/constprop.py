from kirin import ir, types
from kirin.interp import MethodTable, ReturnValue, StatementResult, impl
from kirin.analysis import const
from kirin.dialects.func.stmts import Call, Invoke, Lambda, Return, GetField
from kirin.dialects.func.dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(MethodTable):

    @impl(Return)
    def return_(
        self, interp: const.Propagate, frame: const.Frame, stmt: Return
    ) -> StatementResult[const.Result]:
        return ReturnValue(frame.get(stmt.value))

    @impl(Call)
    def call(
        self, interp: const.Propagate, frame: const.Frame, stmt: Call
    ) -> StatementResult[const.Result]:
        # give up on dynamic method calls
        callee = frame.get(stmt.callee)
        if isinstance(callee, const.PartialLambda):
            call_frame, ret = interp.run_callable(
                callee.code,
                (callee,)
                + interp.permute_values(
                    callee.argnames, frame.get_values(stmt.inputs), stmt.kwargs
                ),
            )
            if not call_frame.frame_is_not_pure:
                frame.should_be_pure.add(stmt)
            return (ret,)

        if not (isinstance(callee, const.Value) and isinstance(callee.data, ir.Method)):
            return (const.Result.bottom(),)

        mt: ir.Method = callee.data
        call_frame, ret = interp.run_method(
            mt,
            interp.permute_values(
                mt.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
            ),
        )
        if not call_frame.frame_is_not_pure:
            frame.should_be_pure.add(stmt)
        return (ret,)

    @impl(Invoke)
    def invoke(
        self,
        interp: const.Propagate,
        frame: const.Frame,
        stmt: Invoke,
    ) -> StatementResult[const.Result]:
        invoke_frame, ret = interp.run_method(
            stmt.callee,
            interp.permute_values(
                stmt.callee.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
            ),
        )
        if not invoke_frame.frame_is_not_pure:
            frame.should_be_pure.add(stmt)
        return (ret,)

    @impl(Lambda)
    def lambda_(
        self, interp: const.Propagate, frame: const.Frame, stmt: Lambda
    ) -> StatementResult[const.Result]:
        captured = frame.get_values(stmt.captured)
        arg_names = [
            arg.name or str(idx) for idx, arg in enumerate(stmt.body.blocks[0].args)
        ]
        if stmt.body.blocks and types.is_tuple_of(captured, const.Value):
            return (
                const.Value(
                    ir.Method(
                        mod=None,
                        py_func=None,
                        sym_name=stmt.sym_name,
                        arg_names=arg_names,
                        dialects=interp.dialects,
                        code=stmt,
                        fields=tuple(each.data for each in captured),
                    )
                ),
            )

        return (
            const.PartialLambda(
                arg_names,
                stmt,
                tuple(each for each in captured),
            ),
        )

    @impl(GetField)
    def getfield(
        self,
        interp: const.Propagate,
        frame: const.Frame,
        stmt: GetField,
    ) -> StatementResult[const.Result]:
        callee_self = frame.get(stmt.obj)
        if isinstance(callee_self, const.Value) and isinstance(
            callee_self.data, ir.Method
        ):
            mt: ir.Method = callee_self.data
            return (const.Value(mt.fields[stmt.field]),)
        elif isinstance(callee_self, const.PartialLambda):
            return (callee_self.captured[stmt.field],)
        return (const.Unknown(),)
