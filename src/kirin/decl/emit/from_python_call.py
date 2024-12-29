import ast
from typing import Any

from typing_extensions import Unpack

from kirin.lowering import Result, LoweringState
from kirin.decl.base import BaseModifier, StatementOptions
from kirin.exceptions import DialectLoweringError

from ._create_fn import create_fn
from ._set_new_attribute import set_new_attribute


class EmitFromPythonCall(BaseModifier):
    _KIRIN_PYATTR = "_kirin_PyAttr"
    _KIRIN_RESULT = "_kirin_Result"
    _LOWERING = "_kirin_Lowering"
    _CALL_NODE = "_kirin_ast_Call_node"
    _CALL_NODE_ARGS = "_kirin_ast_Call_node_args"
    _CALL_NODE_KWARGS = "_kirin_ast_Call_node_kwargs"
    _KIRIN_ERROR = "_kirin_DialectLoweringError"
    _STMT_STD_ARG_NAMES = "_kirin_stmt_std_arg_names"
    _STMT_KW_ARG_NAMES = "_kirin_stmt_kw_arg_names"
    _STMT_ATTR_PROP_NAMES = "_kirin_stmt_attr_prop_names"
    _STMT_REQUIRED_NAMES = "_kirin_stmt_required_names"
    _STMT_GROUP_ARG_NAMES = "_kirin_stmt_group_args_names"
    _AST_TUPLE = "_kirin_ast_Tuple"
    _AST_CONSTANT = "_kirin_ast_Constant"

    def __init__(self, cls: type, **kwargs: Unpack[StatementOptions]) -> None:
        from kirin.dialects.py import data

        super().__init__(cls, **kwargs)
        self.globals[self._KIRIN_RESULT] = Result
        self.globals[self._KIRIN_PYATTR] = data.PyAttr
        self.globals[self._KIRIN_ERROR] = DialectLoweringError
        self.globals[self._AST_TUPLE] = ast.Tuple
        self.globals[self._AST_CONSTANT] = ast.Constant
        self._from_python_call_locals: dict[str, Any] = {}
        self._from_python_call_body: list[str] = []

    def emit_from_python_call(self):
        if self.fields.blocks or self.fields.regions:
            return

        self._from_python_call_locals.update(
            {
                "_kirin_hint_lower": LoweringState,
                "_kirin_hint_node": ast.Call,
            }
        )
        self._emit_body()
        fn = create_fn(
            "from_python_call",
            args=[
                self._class_name,
                f"{self._LOWERING}: _kirin_hint_lower",
                f"{self._CALL_NODE}: _kirin_hint_node",
            ],
            body=self._from_python_call_body,
            globals=self.globals,
            locals=self._from_python_call_locals,
            return_type=Result,
        )
        set_new_attribute(self.cls, "from_python_call", classmethod(fn))

    def _emit_body(self):
        self._from_python_call_locals[self._STMT_STD_ARG_NAMES] = list(
            self.fields.std_args.keys()
        )
        self._from_python_call_locals[self._STMT_KW_ARG_NAMES] = list(
            self.fields.kw_args.keys()
        )
        self._from_python_call_locals[self._STMT_ATTR_PROP_NAMES] = (
            self.fields.attr_or_props
        )
        self._from_python_call_locals[self._STMT_REQUIRED_NAMES] = (
            self.fields.required_names
        )
        self._from_python_call_locals[self._STMT_GROUP_ARG_NAMES] = (
            self.fields.group_arg_names
        )

        self._from_python_call_body.extend(
            [
                f"{self._CALL_NODE_ARGS} = {self._CALL_NODE}.args",
                f"{self._CALL_NODE_KWARGS} = {self._CALL_NODE}.keywords",
                "_kirin_args = {}",
                "_kirin_kwargs = {}",
                f"for _name, _value in zip({self._STMT_STD_ARG_NAMES}, {self._CALL_NODE_ARGS}):",
                *self._parse_arg("_kirin_args", "_name", "_value", indent=1),
                f"for _kw in {self._CALL_NODE_KWARGS}:",
                f"    if _kw.arg in {self._CALL_NODE_ARGS}:",
                f"        raise {self._KIRIN_ERROR}(f'duplicate argument: {{_kw.arg}}')",
                f"    elif _kw.arg in {self._STMT_STD_ARG_NAMES}:",
                *self._parse_arg("_kirin_kwargs", "_kw.arg", "_kw.value", indent=2),
                f"    elif _kw.arg in {self._STMT_KW_ARG_NAMES}:",
                *self._parse_arg("_kirin_kwargs", "_kw.arg", "_kw.value", indent=2),
                f"    elif _kw.arg in {self._STMT_ATTR_PROP_NAMES}:",
                f"        if not isinstance(_kw.value, {self._AST_CONSTANT}):",
                f"            raise {self._KIRIN_ERROR}(f'expected constant for attribute/property {{_kw.arg}}')",
                f"        _kirin_kwargs[_kw.arg] = {self._KIRIN_PYATTR}(_kw.value.value)",
                "    else:",
                f"        raise {self._KIRIN_ERROR}(f'unexpected keyword argument {{_kw.arg}}')",
                f"for _name in {self._STMT_REQUIRED_NAMES}:",
                "    if _name not in _kirin_args and _name not in _kirin_kwargs:",
                f"        raise {self._KIRIN_ERROR}(f'missing required argument {{_name}}')",
                f"_stmt = {self._class_name}(*_kirin_args.values(), **_kirin_kwargs)",
                f"{self._LOWERING}.append_stmt(_stmt)",
                f"return {self._KIRIN_RESULT}(_stmt)",
            ]
        )

    def _parse_arg(
        self, target: str, name: str, value: str, indent: int = 0
    ) -> list[str]:
        lines = [
            f"if {name} in {self._STMT_GROUP_ARG_NAMES}:",
            f"    if not isinstance({value}, {self._AST_TUPLE}):",
            f"        raise {self._KIRIN_ERROR}(f'expected tuple for group argument {{{name}}}')",
            f"    {target}[{name}] = tuple({self._LOWERING}.visit(_elem).expect_one() for _elem in {value}.elts)",
            "else:",
            f"    {target}[{name}] = {self._LOWERING}.visit({value}).expect_one()",
        ]
        return [f"{'    ' * indent}{line}" for line in lines]
