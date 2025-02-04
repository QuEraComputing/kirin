# adopted from mkdocstrings/griffe-pydantic/
# from __future__ import annotations

from typing import Any, cast

import griffe

logger = griffe.get_logger(__name__)


def inherits_statement(cls: griffe.Class) -> bool:
    """Tell whether a class inherits from kirin.ir.Statement."""
    for base in cls.bases:
        if isinstance(base, (griffe.ExprName, griffe.Expr)):
            base = base.canonical_path  # noqa: PLW2901
        if base == "kirin.ir.Statement":
            return True
    return any(inherits_statement(parent_cls) for parent_cls in cls.mro())


def has_default(value: griffe.ExprCall) -> bool:
    return (
        len(value.arguments) >= 1
        and not isinstance(value.arguments[0], griffe.ExprKeyword)
        and value.arguments[0] != "..."
    )  # handle field(...), i.e. no default


def process_attribute(
    attr: griffe.Attribute, cls: griffe.Class, *, processed: set[str]
) -> None:
    """Handle ir.Statement attributes."""
    if attr.canonical_path in processed:
        return
    processed.add(attr.canonical_path)

    kwargs = {}
    if isinstance(attr.value, griffe.ExprCall):
        kwargs = {
            argument.name: argument.value
            for argument in attr.value.arguments
            if isinstance(argument, griffe.ExprKeyword)
        }
        field_descriptor = attr.value.function.canonical_path
        kwargs = _descriptor_args(attr.value)
        if field_descriptor == "kirin.decl.info.argument":
            attr.labels = {"kirin-argument"}
            if "kw_only" in kwargs and kwargs["kw_only"] == "True":
                attr.labels.add("kw-only")
        elif field_descriptor == "kirin.decl.info.attribute":
            if "property" in kwargs and kwargs["property"] == "True":
                attr.labels = {"kirin-property"}
            else:
                attr.labels = {"kirin-attribute"}
            attr.labels.add("kw-only")
        elif field_descriptor == "kirin.decl.info.result":
            attr.labels = {"kirin-result"}
        elif field_descriptor == "kirin.decl.info.region":
            attr.labels = {"kirin-region", "kw-only"}
        elif field_descriptor == "kirin.decl.info.block":
            attr.labels = {"kirin-block", "kw-only"}
    elif attr.value is not None:
        # logger.warning(f"Unhandled attribute value: {attr.value!r}")
        return  # wrong declaration
    elif attr.value is None and _is_annotation_SSAValue(attr.annotation):
        attr.labels = {"kirin-argument"}


def _is_annotation_SSAValue(annotation: str | griffe.Expr | None):
    if annotation is None:
        return False
    if isinstance(annotation, str):
        return annotation in ("ir.SSAValue", "SSAValue")
    if isinstance(annotation, griffe.Expr):
        return annotation.canonical_path in "kirin.ir.ssa.SSAValue"


def _descriptor_args(field: griffe.ExprCall) -> dict[str, Any]:
    kwargs = {
        argument.name: argument.value
        for argument in field.arguments
        if isinstance(argument, griffe.ExprKeyword)
    }
    if has_default(field):
        kwargs["type"] = field.arguments[0]
    return kwargs


def process_class(cls: griffe.Class, *, processed: set[str]) -> None:
    """Handle ir.Statement classes."""
    if cls.canonical_path in processed:
        return

    if not inherits_statement(cls):
        return

    processed.add(cls.canonical_path)
    cls.labels = {"krin-statement"}
    for member in cls.all_members.values():
        if isinstance(member, griffe.Attribute):
            process_attribute(member, cls, processed=processed)
        # TODO: process inherited methods?

    if "__init__" not in cls.members:
        _statement_init(cls)


def _statement_init_parameters(cls: griffe.Class) -> list[griffe.Parameter]:
    params: list[griffe.Parameter] = []
    for member in cls.members.values():
        if member.is_attribute:
            member = cast(griffe.Attribute, member)
            if member.annotation is None:
                continue

            if isinstance(member.value, griffe.ExprCall):
                kwargs = _descriptor_args(member.value)
            else:
                kwargs = {}

            if "init" in kwargs and kwargs["init"] == "False":
                continue

            kind = (
                griffe.ParameterKind.keyword_only
                if "kw-only" in member.labels
                else griffe.ParameterKind.positional_or_keyword
            )

            if "default_factory" in kwargs:
                default_factory = kwargs["default_factory"]
                if isinstance(default_factory, griffe.ExprLambda):
                    default = default_factory.body
                else:
                    default = griffe.ExprCall(
                        function=kwargs["default_factory"], arguments=[]
                    )
            else:
                default = None

            if "kirin-argument" in member.labels:
                params.append(
                    griffe.Parameter(
                        member.name,
                        annotation=member.annotation,
                        kind=kind,
                        default=default,
                        docstring=member.docstring,
                    )
                )
            elif (
                "kirin-attribute" in member.labels
                or "kirin-property" in member.labels
                or "kirin-block" in member.labels
                or "kirin-region" in member.labels
            ):
                params.append(
                    griffe.Parameter(
                        member.name,
                        annotation=member.annotation,
                        kind=griffe.ParameterKind.keyword_only,
                        default=default,
                        docstring=member.docstring,
                    )
                )
    return params


def _statement_init(cls: griffe.Class):
    params = []
    try:
        mro = cls.mro()
    except ValueError:
        mro = ()
    for parent in reversed(mro):
        if inherits_statement(parent):
            params.extend(_statement_init_parameters(parent))
            cls.labels.add("kirin-statement")

    if not inherits_statement(cls):
        return

    params.extend(_statement_init_parameters(cls))

    init = griffe.Function(
        "__init__",
        lineno=0,
        endlineno=0,
        parent=cls,
        parameters=griffe.Parameters(
            griffe.Parameter(
                name="self",
                annotation=None,
                kind=griffe.ParameterKind.positional_or_keyword,
                default=None,
                docstring=None,
            ),
            *_reorder_parameters(params),
        ),
        returns="None",
    )
    cls.set_member("__init__", init)


# NOTE: copied from griffe/src/_griffe/extensions/dataclasses.py
def _reorder_parameters(parameters: list[griffe.Parameter]) -> list[griffe.Parameter]:
    # De-duplicate, overwriting previous parameters.
    params_dict = {param.name: param for param in parameters}

    # Re-order, putting positional-only in front and keyword-only at the end.
    pos_only = []
    pos_kw = []
    kw_only = []
    for param in params_dict.values():
        if param.kind is griffe.ParameterKind.positional_only:
            pos_only.append(param)
        elif param.kind is griffe.ParameterKind.keyword_only:
            kw_only.append(param)
        else:
            pos_kw.append(param)
    return pos_only + pos_kw + kw_only


def process_module(mod: griffe.Module, *, processed: set[str]):
    if mod.canonical_path in processed:
        return

    processed.add(mod.canonical_path)
    for cls in mod.classes.values():
        if not cls.is_alias:
            process_class(cls, processed=processed)

    for submod in mod.modules.values():
        if not submod.is_alias:
            process_module(submod, processed=processed)


class KirinExtension(griffe.Extension):

    def __init__(self):
        super().__init__()
        self.in_statement: list[griffe.Class] = []
        self.processed: set[str] = set()

    def on_package_loaded(
        self, *, pkg: griffe.Module, loader: griffe.GriffeLoader, **kwargs: Any
    ) -> None:
        process_module(pkg, processed=self.processed)


# extensions = griffe.load_extensions(KirinExtension())
# data = griffe.load("kirin", extensions=extensions)

# obj = data["dialects.func.stmts.Function"]
# print(obj.labels)
# # print(obj.get_member("sym_name").labels)
# # print(obj.get_member("body").labels)
# # print(obj.get_member("sym_name").docstring.value)
