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
class FromPythonCall(PythonLoweringTrait[StatementType, ast.Call]):
    """Trait for customizing lowering of Python calls to a statement.

    Declared in a statement definition to indicate that the statement can be
    constructed from a Python call (i.e., a function call `ast.Call` in the
    Python AST).

    Subclassing this trait allows for customizing the lowering of Python calls
    to the statement. The `lower` method should be implemented to parse the
    arguments from the Python call and construct the statement instance.
    """

    def lower(
        self, stmt: type[StatementType], state: "LoweringState", node: ast.Call
    ) -> "Result":
        return state.default_Call_lower(stmt, node)

    def verify(self, stmt: "Statement"):
        assert len(stmt.regions) == 0, "FromPythonCall statements cannot have regions"
        assert (
            len(stmt.successors) == 0
        ), "FromPythonCall statements cannot have successors"


@dataclass(frozen=True)
class FromPythonRangeLike(FromPythonCall[StatementType]):
    """Provides a default lowering implementation for built-in `range`-like function
    to a statement that takes three arguments: start, stop, and step.
    """

    def lower(
        self, stmt: type[StatementType], state: "LoweringState", node: ast.Call
    ) -> "Result":
        from kirin.lowering import Result

        if len(node.args) == 1:
            start = state.visit(ast.Constant(0)).expect_one()
            stop = state.visit(node.args[0]).expect_one()
            step = state.visit(ast.Constant(1)).expect_one()
        elif len(node.args) == 2:
            start = state.visit(node.args[0]).expect_one()
            stop = state.visit(node.args[1]).expect_one()
            step = state.visit(ast.Constant(1)).expect_one()
        elif len(node.args) == 3:
            start = state.visit(node.args[0]).expect_one()
            stop = state.visit(node.args[1]).expect_one()
            step = state.visit(node.args[2]).expect_one()
        else:
            raise DialectLoweringError("range() takes 1-3 arguments")

        return Result(state.append_stmt(stmt(start, stop, step)))  # type: ignore
