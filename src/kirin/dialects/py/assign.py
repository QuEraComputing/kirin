"""Assignment dialect for Python.

This module contains the dialect for the Python assignment statement, including:

- Statements: `Alias`, `SetItem`.
- The lowering pass for the assignments.
- The concrete implementation of the assignment statements.

This dialects maps Python assignment syntax.
"""

import ast

from kirin import ir, types, interp, lowering, exceptions
from kirin.decl import info, statement
from kirin.print import Printer

dialect = ir.Dialect("py.assign")

T = types.TypeVar("T")


@statement(dialect=dialect)
class Alias(ir.Statement):
    name = "alias"
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    value: ir.SSAValue = info.argument(T)
    target: ir.PyAttr[str] = info.attribute()
    result: ir.ResultValue = info.result(T)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        with printer.rich(style="symbol"):
            printer.plain_print(self.target.data)

        with printer.rich(style="keyword"):
            printer.plain_print(" = ")

        printer.print(self.value)


@statement(dialect=dialect)
class SetItem(ir.Statement):
    name = "setitem"
    traits = frozenset({ir.FromPythonCall()})
    obj: ir.SSAValue = info.argument(print=False)
    value: ir.SSAValue = info.argument(print=False)
    index: ir.SSAValue = info.argument(print=False)


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Alias)
    def alias(self, interp, frame: interp.Frame, stmt: Alias):
        return (frame.get(stmt.value),)

    @interp.impl(SetItem)
    def setindex(self, interp, frame: interp.Frame, stmt: SetItem):
        frame.get(stmt.obj)[frame.get(stmt.index)] = frame.get(stmt.value)
        return (None,)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Assign(
        self, state: lowering.LoweringState, node: ast.Assign
    ) -> lowering.Result:
        results: lowering.Result = state.visit(node.value)
        assert len(node.targets) == len(
            results
        ), "number of targets and results do not match"

        current_frame = state.current_frame
        match node:
            case ast.Assign(
                targets=[ast.Name(lhs_name, ast.Store())], value=ast.Name(_, ast.Load())
            ):
                stmt = Alias(
                    value=results[0], target=ir.PyAttr(lhs_name)
                )  # NOTE: this is guaranteed to be one result
                stmt.result.name = lhs_name
                current_frame.defs[lhs_name] = state.append_stmt(stmt).result
            case _:
                for target, value in zip(node.targets, results.values):
                    match target:
                        # NOTE: if the name exists new ssa value will be
                        # used in the future to shadow the old one
                        case ast.Name(name, ast.Store()):
                            value.name = name
                            current_frame.defs[name] = value
                        case ast.Subscript(obj, slice):
                            obj = state.visit(obj).expect_one()
                            slice = state.visit(slice).expect_one()
                            stmt = SetItem(obj=obj, index=slice, value=value)
                            state.append_stmt(stmt)
                        case _:
                            raise exceptions.DialectLoweringError(
                                f"unsupported target {target}"
                            )
        return lowering.Result()  # python assign does not have value
