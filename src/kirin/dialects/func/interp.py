from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Call, GetField, Invoke, Lambda, Return
from kirin.interp import DialectInterpreter, ResultValue, ReturnValue, concrete, impl
from kirin.ir import Method


@dialect.register
class Interpreter(DialectInterpreter):

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

    @impl(GetField)
    def getfield(self, interp: concrete.Interpreter, stmt: GetField, values: tuple):
        mt: Method = values[0]
        return ResultValue(mt.fields[stmt.field])

    @impl(Lambda)
    def lambda_(self, interp: concrete.Interpreter, stmt: Lambda, values: tuple):
        return ResultValue(
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
            )
        )
