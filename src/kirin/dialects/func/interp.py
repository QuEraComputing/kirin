from kirin.ir import Method
from kirin.interp import ReturnValue, DialectMethodTable, impl, concrete
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
class Interpreter(DialectMethodTable):

    @impl(Call)
    def call(self, interp: concrete.Interpreter, stmt: Call, values: tuple):
        mt: Method = values[0]
        return interp.eval(
            mt, interp.permute_values(mt.arg_names, values[1:], stmt.kwargs)
        ).to_result()

    @impl(Invoke)
    def invoke(self, interp: concrete.Interpreter, stmt: Invoke, values: tuple):
        return interp.eval(
            stmt.callee,
            interp.permute_values(stmt.callee.arg_names, values, stmt.kwargs),
        ).to_result()

    @impl(Return)
    def return_(self, interp: concrete.Interpreter, stmt: Return, values: tuple):
        return ReturnValue(values[0])

    @impl(ConstantNone)
    def const_none(
        self, interp: concrete.Interpreter, stmt: ConstantNone, values: tuple[()]
    ):
        return (None,)

    @impl(GetField)
    def getfield(self, interp: concrete.Interpreter, stmt: GetField, values: tuple):
        mt: Method = values[0]
        return (mt.fields[stmt.field],)

    @impl(Lambda)
    def lambda_(self, interp: concrete.Interpreter, stmt: Lambda, values: tuple):
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
                fields=values,
            ),
        )
