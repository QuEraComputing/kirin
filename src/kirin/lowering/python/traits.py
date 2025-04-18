from __future__ import annotations

import ast
from abc import abstractmethod
from typing import Any, Generic, TypeVar, get_origin
from dataclasses import dataclass

from kirin import ir
from kirin.decl import fields
from kirin.lowering.abc import Result
from kirin.lowering.state import State
from kirin.lowering.exception import BuildError

ASTNode = TypeVar("ASTNode", bound=ast.AST)
StmtType = TypeVar("StmtType", bound="ir.Statement")


@dataclass(frozen=True)
class PythonLoweringTrait(ir.Trait[ir.Statement], Generic[StmtType, ASTNode]):
    """A trait that indicates that a statement can be lowered from Python AST."""

    @abstractmethod
    def lower(
        self, stmt: type[StmtType], state: State[ast.AST], node: ASTNode
    ) -> Result: ...

    @classmethod
    def lower_Call_inputs(
        cls, stmt: type[StmtType], state: State[ast.AST], node: ast.Call
    ) -> tuple[dict[str, ir.SSAValue | tuple[ir.SSAValue, ...]], dict[str, Any]]:
        """Lower the inputs of a Python call to corresponding SSA values or
        compile-time values (attributes).

        Args:
            stmt: The statement class to lower to.
            state: The lowering state.
            node: The Python call node to lower.

        Returns:
            A tuple containing two dictionaries:
            - The first dictionary contains the standard arguments and their values.
            - The second dictionary contains the keyword arguments and their values.

        Raises:
            lowering.BuildError: If the Python call cannot be lowered to the statement.
        """
        fs = fields(stmt)
        stmt_std_arg_names = fs.std_args.keys()
        stmt_kw_args_name = fs.kw_args.keys()
        stmt_attr_prop_names = fs.attr_or_props
        stmt_required_names = fs.required_names
        stmt_group_arg_names = fs.group_arg_names
        args, kwargs = {}, {}
        for name, value in zip(stmt_std_arg_names, node.args):
            cls.__parse_arg(state, stmt_group_arg_names, args, name, value)
        for kw in node.keywords:
            if not isinstance(kw.arg, str):
                raise BuildError("Expected string for keyword argument name")

            arg: str = kw.arg
            if arg in node.args:
                raise BuildError(
                    f"Keyword argument {arg} is already present in positional arguments"
                )
            elif arg in stmt_std_arg_names or arg in stmt_kw_args_name:
                cls.__parse_arg(state, stmt_group_arg_names, kwargs, kw.arg, kw.value)
            elif arg in stmt_attr_prop_names:
                if (
                    isinstance(kw.value, ast.Name)
                    and state.current_frame.get_local(kw.value.id) is not None
                ):
                    raise BuildError(
                        f"Expected global/constant value for attribute or property {arg}"
                    )
                global_value = state.get_global(kw.value)
                if (decl := fs.attributes.get(arg)) is not None:
                    if decl.annotation is Any:
                        kwargs[arg] = global_value.data
                    else:
                        kwargs[arg] = global_value.expect(
                            get_origin(decl.annotation) or decl.annotation
                        )
                else:
                    raise BuildError(f"Unexpected attribute or property {arg}")
            else:
                raise BuildError(f"Unexpected keyword argument {arg}")

        for name in stmt_required_names:
            if name not in args and name not in kwargs:
                raise BuildError(f"Missing required argument {name}")

        return args, kwargs

    @classmethod
    def __parse_arg(
        cls,
        state: State[ast.AST],
        group_names: set[str],
        target: dict,
        name: str,
        value: ast.AST,
    ):
        if name in group_names:
            if not isinstance(value, ast.Tuple):
                raise BuildError(f"Expected tuple for group argument {name}")
            target[name] = tuple(state.lower(elem).expect_one() for elem in value.elts)
        else:
            target[name] = state.lower(value).expect_one()


StatementType = TypeVar("StatementType", bound="ir.Statement")


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
        self, stmt: type[StatementType], state: State[ast.AST], node: ast.Call
    ) -> Result:
        args, kwargs = self.lower_Call_inputs(stmt, state, node)
        return state.current_frame.push(stmt(*args.values(), **kwargs))

    def verify(self, node: ir.Statement):
        assert len(node.regions) == 0, "FromPythonCall statements cannot have regions"
        assert (
            len(node.successors) == 0
        ), "FromPythonCall statements cannot have successors"


@dataclass(frozen=True)
class FromPythonRangeLike(FromPythonCall[StatementType]):
    """Provides a default lowering implementation for built-in `range`-like function
    to a statement that takes three arguments: start, stop, and step.
    """

    def lower(
        self, stmt: type[StatementType], state: State[ast.AST], node: ast.Call
    ) -> Result:
        nargs = len(node.args)
        if nargs == 1:
            start = state.get_literal(0)
            stop = state.lower(node.args[0]).expect_one()
            step = state.get_literal(1)
        elif nargs == 2:
            start = state.lower(node.args[0]).expect_one()
            stop = state.lower(node.args[1]).expect_one()
            step = state.get_literal(1)
        elif nargs == 3:
            start = state.lower(node.args[0]).expect_one()
            stop = state.lower(node.args[1]).expect_one()
            step = state.lower(node.args[2]).expect_one()
        else:
            raise BuildError("range() takes 1-3 arguments")

        return state.current_frame.push(stmt(start, stop, step))  # type: ignore


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
        self, stmt: type[StatementType], state: State[ast.AST], node: ast.With
    ) -> Result:
        from kirin.dialects import cf

        fs = fields(stmt)
        if len(fs.regions) != 1:
            raise BuildError("Expected exactly one region in statement declaration")

        if len(node.items) != 1:
            raise BuildError("Expected exactly one item in statement")

        item, body = node.items[0], node.body
        if not isinstance(item.context_expr, ast.Call):
            raise BuildError(
                f"Expected context expression to be a call for with {stmt.name}"
            )

        region_name, region_info = next(iter(fs.regions.items()))
        if region_info.multi:  # branch to exit block if not terminated
            with state.frame(body) as body_frame:
                body_frame.exhaust()
                for block in body_frame.curr_region.blocks:
                    if block.last_stmt is None or not block.last_stmt.has_trait(
                        ir.IsTerminator
                    ):
                        block.stmts.append(
                            cf.Branch(arguments=(), successor=body_frame.next_block)
                        )
                for block in body_frame.curr_region.blocks:
                    if block.last_stmt is None or not block.last_stmt.has_trait(
                        ir.IsTerminator
                    ):
                        block.stmts.append(
                            cf.Branch(arguments=(), successor=body_frame.next_block)
                        )
        else:
            with state.frame(body, finalize_next=False) as body_frame:
                body_frame.exhaust()
                if len(body_frame.curr_region.blocks) != 1:
                    raise BuildError(
                        f"Expected exactly one block in region {region_name}"
                    )

                if len(body_frame.curr_region.blocks) != 1:
                    raise BuildError(
                        f"Expected exactly one block in region {region_name}"
                    )

        args, kwargs = self.lower_Call_inputs(stmt, state, item.context_expr)
        kwargs[region_name] = body_frame.curr_region
        results = state.current_frame.push(stmt(*args.values(), **kwargs)).results
        if len(results) == 0:
            return
        elif len(results) > 1:
            raise BuildError(
                f"Expected exactly one result or no result from statement {stmt.name}"
            )

        result = results[0]
        if item.optional_vars is not None and isinstance(item.optional_vars, ast.Name):
            result.name = item.optional_vars.id
            state.current_frame.defs[result.name] = result
        return

    def verify(self, node: ir.Statement):
        assert (
            len(node.regions) == 1
        ), "FromPythonWithSingleItem statements must have one region"
        assert (
            len(node.successors) == 0
        ), "FromPythonWithSingleItem statements cannot have successors"
        assert (
            len(node.results) <= 1
        ), "FromPythonWithSingleItem statements can have at most one result"
