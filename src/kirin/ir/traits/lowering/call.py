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

    def lower(
        self, stmt: type[StatementType], state: "LoweringState", node: ast.Call
    ) -> "Result":
        from kirin.decl import fields
        from kirin.lowering import Result
        from kirin.dialects.py.data import PyAttr

        fs = fields(stmt)
        stmt_std_arg_names = fs.std_args.keys()
        stmt_kw_args_name = fs.kw_args.keys()
        stmt_attr_prop_names = fs.attr_or_props
        stmt_required_names = fs.required_names
        stmt_group_arg_names = fs.group_arg_names
        args, kwargs = {}, {}
        for name, value in zip(stmt_std_arg_names, node.args):
            self._parse_arg(stmt_group_arg_names, state, args, name, value)
        for kw in node.keywords:
            if not isinstance(kw.arg, str):
                raise DialectLoweringError("Expected string for keyword argument name")

            arg: str = kw.arg
            if arg in node.args:
                raise DialectLoweringError(
                    f"Keyword argument {arg} is already present in positional arguments"
                )
            elif arg in stmt_std_arg_names or arg in stmt_kw_args_name:
                self._parse_arg(stmt_group_arg_names, state, kwargs, kw.arg, kw.value)
            elif arg in stmt_attr_prop_names:
                if not isinstance(kw.value, ast.Constant):
                    raise DialectLoweringError(
                        f"Expected constant for attribute or property {arg}"
                    )
                kwargs[arg] = PyAttr(kw.value.value)
            else:
                raise DialectLoweringError(f"Unexpected keyword argument {arg}")

        for name in stmt_required_names:
            if name not in args and name not in kwargs:
                raise DialectLoweringError(f"Missing required argument {name}")

        return Result(state.append_stmt(stmt(*args.values(), **kwargs)))

    @staticmethod
    def _parse_arg(
        group_names: set[str],
        state: "LoweringState",
        target: dict,
        name: str,
        value: ast.AST,
    ):
        if name in group_names:
            if not isinstance(value, ast.Tuple):
                raise DialectLoweringError(f"Expected tuple for group argument {name}")
            target[name] = tuple(state.visit(elem).expect_one() for elem in value.elts)
        else:
            target[name] = state.visit(value).expect_one()
