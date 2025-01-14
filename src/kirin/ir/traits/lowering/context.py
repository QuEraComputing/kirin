"""Traits for customizing lowering of Python `with` syntax to a statement.
"""

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
    """Trait for customizing lowering of Python with statements to a statement.

    Subclassing this trait allows for customizing the lowering of Python with
    statements to the statement. The `lower` method should be implemented to parse
    the arguments from the Python with statement and construct the statement instance.
    """

    pass


@dataclass(frozen=True)
class FromPythonWithSingleItem(FromPythonWith[StatementType]):
    """Trait for customizing lowering of the following Python with syntax to a statement:

    ```python
    with <stmt>[ as <name>]:
        <body>
    ```

    where `<stmt>` is the statement being lowered, `<name>` is an optional name for the result
    of the statement, and `<body>` is the body of the with statement. The optional `as <name>`
    is not valid when the statement has no results.

    This syntax is slightly different from the standard Python `with` statement in that
    `<name>` refers to the result of the statement, not the context manager. Thus typically
    one sould access `<name>` in `<body>` to use the result of the statement.

    In some cases, however, `<name>` may be used as a reference of a special value `self` that
    is passed to the `<body>` of the statement. This is useful for statements that have a similar
    behavior to a closure.
    """

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
        results = state.append_stmt(stmt(*args.values(), **kwargs)).results
        if len(results) == 0:
            return lowering.Result()
        elif len(results) > 1:
            raise DialectLoweringError(
                f"Expected exactly one result or no result from statement {stmt.name}"
            )

        result = results[0]
        if item.optional_vars is not None and isinstance(item.optional_vars, ast.Name):
            result.name = item.optional_vars.id
            state.current_frame.defs[result.name] = result
        return lowering.Result(result)
