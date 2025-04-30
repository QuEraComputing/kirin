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
        _, ret = interp.call(
            mt.code,
            mt,
            *frame.get_values(stmt.inputs),
            **{k: v for k, v in zip(stmt.keys, frame.get_values(stmt.kwargs))},
        )
        return (ret,)

    @impl(Invoke)
    def invoke(self, interp: concrete.Interpreter, frame: Frame, stmt: Invoke):
        _, ret = interp.call(
            stmt.callee,
            *frame.get_values(stmt.inputs),
            **{k: v for k, v in zip(stmt.keys, frame.get_values(stmt.kwargs))},
        )
        return (ret,)

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
