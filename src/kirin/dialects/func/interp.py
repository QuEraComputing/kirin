from kirin.ir import Method
from kirin.interp import Frame, MethodTable, ReturnValue, impl, concrete
from kirin.dialects.func.stmts import (
    Call,
    Invoke,
    Lambda,
    Return,
    GetField,
    ConstantNone,
)
from kirin.dialects.func.dialect import dialect


@dialect.register
class Interpreter(MethodTable):

    @impl(Call)
    def call(self, interp: concrete.Interpreter, frame: Frame, stmt: Call):
        mt: Method = frame.get(stmt.callee)
        _, result = interp.run_method(
            mt,
            interp.permute_values(
                mt.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
            ),
        )
        return (result,)

    @impl(Invoke)
    def invoke(self, interp: concrete.Interpreter, frame: Frame, stmt: Invoke):
        _, result = interp.run_method(
            stmt.callee,
            interp.permute_values(
                stmt.callee.arg_names, frame.get_values(stmt.inputs), stmt.kwargs
            ),
        )
        return (result,)

    @impl(Return)
    def return_(self, interp: concrete.Interpreter, frame: Frame, stmt: Return):
        return ReturnValue(frame.get(stmt.value))

    @impl(ConstantNone)
    def const_none(
        self, interp: concrete.Interpreter, frame: Frame, stmt: ConstantNone
    ):
        return (None,)

    @impl(GetField)
    def getfield(self, interp: concrete.Interpreter, frame: Frame, stmt: GetField):
        mt: Method = frame.get(stmt.obj)
        return (mt.fields[stmt.field],)

    @impl(Lambda)
    def lambda_(self, interp: concrete.Interpreter, frame: Frame, stmt: Lambda):
        return (
            Method(
                mod=None,
                py_func=None,
                sym_name=stmt.name,
                arg_names=[
                    arg.name or str(idx)
                    for idx, arg in enumerate(stmt.body.blocks[0].args)
                ],
                dialects=interp.dialects,
                code=stmt,
                fields=frame.get_values(stmt.captured),
            ),
        )
