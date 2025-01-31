import ast
from kirin import ir, lowering
from kirin.decl import statement, info
from kirin.print import Printer
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("py.unpack")


@statement(dialect=dialect, init=False)
class Unpack(ir.Statement):
    value: ir.SSAValue = info.argument(ir.types.Any)
    names: tuple[str | None, ...] = info.attribute(property=True)

    def __init__(self, value: ir.SSAValue, names: tuple[str | None, ...]):
        result_types = [ir.types.Any] * len(names)
        super().__init__(
            args=(value,),
            result_types=result_types,
            args_slice={"value": 0},
            properties={"names": ir.PyAttr(names)},
        )
        for result, name in zip(self.results, names):
            result.name = name

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        printer.print(self.value)


def unpackable(state: lowering.LoweringState, node: ast.expr, value: ir.SSAValue):
    if isinstance(node, ast.Name):
        state.current_frame.defs[node.id] = value
        return
    elif not isinstance(node, ast.Tuple):
        raise DialectLoweringError(f"unsupported unpack node {node}")

    names: list[str | None] = []
    continue_unpack: list[int] = []
    for idx, item in enumerate(node.elts):
        if isinstance(item, ast.Name):
            names.append(item.id)
        else:
            names.append(None)
            continue_unpack.append(idx)
    stmt = state.append_stmt(Unpack(value, tuple(names)))
    for idx in continue_unpack:
        unpackable(state, node.elts[idx], stmt.results[idx])
