"""The unpack dialect for Python.

This module contains the dialect for the Python unpack semantics, including:

- The `Unpack` statement class.
- The lowering pass for the unpack statement.
- The concrete implementation of the unpack statement.
- The type inference implementation of the unpack statement.
- A helper function `unpacking` for unpacking Python AST nodes during lowering.
"""

import ast

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.print import Printer
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("py.unpack")


@statement(dialect=dialect, init=False)
class Unpack(ir.Statement):
    value: ir.SSAValue = info.argument(types.Any)
    names: tuple[str | None, ...] = info.attribute()

    def __init__(self, value: ir.SSAValue, names: tuple[str | None, ...]):
        result_types = [types.Any] * len(names)
        super().__init__(
            args=(value,),
            result_types=result_types,
            args_slice={"value": 0},
            attributes={"names": ir.PyAttr(names)},
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
    def unpack(self, interp, frame: interp.Frame[types.TypeAttribute], stmt: Unpack):
        value = frame.get(stmt.value)
        if isinstance(value, types.Generic) and value.is_subseteq(types.Tuple):
            if value.vararg:
                rest = tuple(value.vararg.typ for _ in stmt.names[len(value.vars) :])
                return tuple(value.vars) + rest
            else:
                return value.vars
        # TODO: support unpacking other types
        return tuple(types.Any for _ in stmt.names)


def unpacking(state: lowering.LoweringState, node: ast.expr, value: ir.SSAValue):
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
        unpacking(state, node.elts[idx], stmt.results[idx])
