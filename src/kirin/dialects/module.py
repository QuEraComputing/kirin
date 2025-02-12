"""Module dialect provides a simple module
that is roughly a list of function statements.

This dialect provides the dialect necessary for compiling a function into
lower-level IR with all its callee functions.
"""

from kirin import ir, types, interp
from kirin.decl import info, statement
from kirin.print import Printer
from kirin.analysis import TypeInference
from kirin.exceptions import VerificationError

from ._pprint_helper import pprint_calllike

dialect = ir.Dialect("module")


@statement(dialect=dialect)
class Module(ir.Statement):
    traits = frozenset(
        {ir.IsolatedFromAbove(), ir.SymbolTable(), ir.SymbolOpInterface()}
    )
    sym_name: str = info.attribute()
    entry: str = info.attribute()
    body: ir.Region = info.region(multi=False)


@statement(dialect=dialect)
class Invoke(ir.Statement):
    """A special statement that represents
    a function calling functions by symbol name.

    Note:
        This statement is here for completeness, for interpretation,
        it is recommended to rewrite this statement into a `func.Invoke`
        after looking up the symbol table.
    """

    callee: str = info.attribute()
    inputs: tuple[ir.SSAValue, ...] = info.argument()
    kwargs: tuple[str, ...] = info.attribute()
    result: ir.ResultValue = info.result()

    def print_impl(self, printer: Printer) -> None:
        pprint_calllike(self, self.callee, printer)

    def verify(self) -> None:
        if self.kwargs:
            for name in self.kwargs:
                if name not in self.callee:
                    raise VerificationError(
                        self,
                        f"method {self.callee} does not have argument {name}",
                    )
        elif len(self.callee) - 1 != len(self.args):
            raise VerificationError(
                self,
                f"expected {len(self.callee)} arguments, got {len(self.args)}",
            )


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Module)
    def interp_Module(
        self, interp: interp.Interpreter, frame: interp.Frame, stmt: Module
    ):
        for stmt_ in stmt.body.blocks[0].stmts:
            if (trait := stmt.get_trait(ir.SymbolOpInterface)) is not None:
                interp.symbol_table[trait.get_sym_name(stmt_).data] = stmt_
        return ()

    @interp.impl(Invoke)
    def interp_Invoke(
        self, interpreter: interp.Interpreter, frame: interp.Frame, stmt: Invoke
    ):
        callee = interpreter.symbol_table.get(stmt.callee)
        if callee is None:
            raise interp.InterpreterError(f"symbol {stmt.callee} not found")

        trait = callee.get_trait(ir.CallableStmtInterface)
        if trait is None:
            raise interp.InterpreterError(
                f"{stmt.callee} is not callable, got {callee.__class__.__name__}"
            )

        body = trait.get_callable_region(callee)
        mt = ir.Method(
            mod=None,
            py_func=None,
            sym_name=stmt.callee,
            arg_names=[
                arg.name or str(idx) for idx, arg in enumerate(body.blocks[0].args)
            ],
            dialects=interpreter.dialects,
            code=stmt,
        )
        return interpreter.run_method(mt, frame.get_values(stmt.inputs))


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(Module)
    def typeinfer_Module(
        self, interp: TypeInference, frame: interp.Frame, stmt: Module
    ):
        for stmt_ in stmt.body.blocks[0].stmts:
            if (trait := stmt.get_trait(ir.SymbolOpInterface)) is not None:
                interp.symbol_table[trait.get_sym_name(stmt_).data] = stmt_
        return ()

    @interp.impl(Invoke)
    def typeinfer_Invoke(
        self, interp: TypeInference, frame: interp.Frame, stmt: Invoke
    ):
        callee = interp.symbol_table.get(stmt.callee)
        if callee is None:
            return (types.Bottom,)

        trait = callee.get_trait(ir.CallableStmtInterface)
        if trait is None:
            return (types.Bottom,)

        body = trait.get_callable_region(callee)
        mt = ir.Method(
            mod=None,
            py_func=None,
            sym_name=stmt.callee,
            arg_names=[
                arg.name or str(idx) for idx, arg in enumerate(body.blocks[0].args)
            ],
            dialects=interp.dialects,
            code=stmt,
        )
        interp.run_method(mt, mt.arg_types)
        return tuple(result.type for result in callee.results)
