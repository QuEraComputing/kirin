import ast
from typing import TYPE_CHECKING, TypeVar
from dataclasses import dataclass

from kirin.exceptions import DialectLoweringError

from ..abc import PythonLoweringTrait

if TYPE_CHECKING:
    from kirin.ir import Statement
    from kirin.lowering import Result, LoweringState

StatementType = TypeVar("StatementType", bound="Statement")


@dataclass(frozen=True)
class FromPythonWith(PythonLoweringTrait[StatementType, ast.With]):
    pass


@dataclass(frozen=True)
class FromPythonWithSingleItem(FromPythonWith[StatementType]):

    def lower(
        self, stmt: type[StatementType], state: "LoweringState", node: ast.With
    ) -> "Result":
        from kirin import lowering
        from kirin.decl import fields

        fs = fields(stmt)
        if len(fs.regions) != 1:
            raise DialectLoweringError(
                "Expected exactly one region in statement declaration"
            )

        if len(node.items) != 1:
            raise DialectLoweringError("Expected exactly one item in statement")

        item, body = node.items[0], node.body
        if not isinstance(item.context_expr, ast.Call):
            raise DialectLoweringError(
                f"Expected context expression to be a call for with {stmt.name}"
            )

        body_frame = lowering.Frame.from_stmts(body, state, parent=state.current_frame)
        state.push_frame(body_frame)
        state.exhaust()
        state.pop_frame()

        args, kwargs = state.default_Call_inputs(stmt, item.context_expr)
        (region_name,) = fs.regions
        kwargs[region_name] = body_frame.current_region
        return lowering.Result(state.append_stmt(stmt(*args.values(), **kwargs)))
