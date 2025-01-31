import ast

from kirin import ir, interp, lowering
from kirin.decl import info, statement
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


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Unpack)
    def unpack(self, interp: interp.Interpreter, frame: interp.Frame, stmt: Unpack):
        return tuple(frame.get(stmt.value))


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(Unpack)
    def unpack(self, interp, frame: interp.Frame[ir.types.TypeAttribute], stmt: Unpack):
        value = frame.get(stmt.value)
        if isinstance(value, ir.types.Generic) and value.is_subseteq(ir.types.Tuple):
            if value.vararg:
                rest = tuple(value.vararg.typ for _ in stmt.names[len(value.vars) :])
                return tuple(value.vars) + rest
            else:
                return value.vars
        # TODO: support unpacking other types
        return tuple(ir.types.Any for _ in stmt.names)


def unpackable(state: lowering.LoweringState, node: ast.expr, value: ir.SSAValue):
    if isinstance(node, ast.Name):
        state.current_frame.defs[node.id] = value
        value.name = node.id
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
    for name, result in zip(names, stmt.results):
        if name is not None:
            state.current_frame.defs[name] = result

    for idx in continue_unpack:
        unpackable(state, node.elts[idx], stmt.results[idx])
