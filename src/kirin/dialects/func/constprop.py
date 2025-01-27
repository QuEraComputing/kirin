from kirin import ir
from kirin.interp import FrameABC, MethodTable, ReturnValue, StatementResult, impl
from kirin.analysis import const
from kirin.dialects.func.stmts import Call, Invoke, Lambda, Return, GetField
from kirin.dialects.func.dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(MethodTable):

    @impl(Return)
    def return_(
        self, interp: const.Propagate, frame: FrameABC, stmt: Return
    ) -> StatementResult[const.JointResult]:
        return ReturnValue(frame.get(stmt.value))

    @impl(Call)
    def call(
        self, interp: const.Propagate, frame: FrameABC[const.JointResult], stmt: Call
    ) -> StatementResult[const.JointResult]:
        # give up on dynamic method calls
        callee = frame.get(stmt.callee).const
        if isinstance(callee, const.PartialLambda):
            return (
                self._call_lambda(
                    interp,
                    callee,
                    interp.permute_values(
                        callee.argnames, frame.get_values(stmt.inputs), stmt.kwargs
                    ),
                ),
            )

        if not isinstance(callee, const.Value):
            return (const.JointResult.bottom(),)

        mt: ir.Method = callee.data
        return (
            interp.run_method(
                mt,
                interp.permute_values(
                    mt.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
                ),
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
        return interp.run_method(mt, args)

    @impl(Invoke)
    def invoke(
        self,
        interp: const.Propagate,
        frame: FrameABC[const.JointResult],
        stmt: Invoke,
    ) -> StatementResult[const.JointResult]:
        return (
            interp.run_method(
                stmt.callee,
                interp.permute_values(
                    stmt.callee.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
                ),
            ),
        )

    @impl(Lambda)
    def lambda_(
        self, interp: const.Propagate, frame: FrameABC[const.JointResult], stmt: Lambda
    ) -> StatementResult[const.JointResult]:
        captured = frame.get_values(stmt.captured)
        arg_names = [
            arg.name or str(idx) for idx, arg in enumerate(stmt.body.blocks[0].args)
        ]
        if stmt.body.blocks and ir.types.is_tuple_of(captured, const.Value):
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
                            fields=tuple(each.data for each in captured),
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
                    tuple(each.const for each in captured),
                ),
                const.Pure(),
            ),
        )

    @impl(GetField)
    def getfield(
        self,
        interp: const.Propagate,
        frame: FrameABC[const.JointResult],
        stmt: GetField,
    ) -> StatementResult[const.JointResult]:
        callee_self = frame.get(stmt.obj).const
        if isinstance(callee_self, const.Value):
            mt: ir.Method = callee_self.data
            return (
                const.JointResult(const.Value(mt.fields[stmt.field]), const.Pure()),
            )
        elif isinstance(callee_self, const.PartialLambda):
            return (const.JointResult(callee_self.captured[stmt.field], const.Pure()),)
        return (const.JointResult(const.Unknown(), const.Pure()),)
