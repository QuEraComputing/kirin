import ast

from kirin import ir, interp, lowering, exceptions
from kirin.decl import info, statement
from kirin.print import Printer
from kirin.dialects.py import data

dialect = ir.Dialect("py.assign")

T = ir.types.TypeVar("T")


@statement(dialect=dialect)
class Alias(ir.Statement):
    name = "alias"
    traits = frozenset({ir.Pure()})
    value: ir.SSAValue = info.argument(T)
    target: data.PyAttr[str] = info.attribute(property=True)
    result: ir.ResultValue = info.result(T)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        with printer.rich(style=printer.color.symbol):
            printer.plain_print(self.target.data)

        with printer.rich(style=printer.color.keyword):
            printer.plain_print(" = ")

        printer.print(self.value)


@statement(dialect=dialect)
class SetItem(ir.Statement):
    name = "setitem"
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
                    value=results[0], target=data.PyAttr(lhs_name)
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
