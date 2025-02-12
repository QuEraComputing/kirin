import re
import sys
import inspect
import dataclasses
from types import ModuleType
from typing import Callable, get_args
from dataclasses import KW_ONLY

from beartype.door import is_subhint

from kirin import ir, types

from .base import BaseModifier
from .info import Field, ArgumentField, AttributeField, StatementFields, argument


class ScanFields(BaseModifier):
    _FIELDS = "__kirin_stmt_fields"
    # String regex that string annotations for ClassVar or InitVar must match.
    # Allows "identifier.identifier[" or "identifier[".
    # https://bugs.python.org/issue33453 for details.
    _MODULE_IDENTIFIER_RE = re.compile(r"^(?:\s*(\w+)\s*\.)?\s*(\w+)")

    def scan_fields(self):
        self._scan_base_fields()
        self._scan_annotations()
        setattr(self.cls, self._FIELDS, self.fields)
        return

    def _scan_base_fields(self):
        # let's just assume dict preserve insertion order in python 3.7+
        for b in self.cls.__mro__[-1:0:-1]:  # taken from dataclass impl
            base_fields: StatementFields | None = getattr(b, self._FIELDS, None)
            if base_fields is not None:
                self.has_statement_bases = True
                for f in base_fields:
                    assert f.name is not None, "field name must be set"
                    self.fields[f.name] = f

    def _scan_annotations(self):
        cls_fields: list[Field] = []
        cls_annotations = inspect.get_annotations(self.cls, eval_str=True)
        for name, typ in cls_annotations.items():
            # See if this is a marker to change the value of kw_only.
            if self._is_kw_only(typ) or (
                isinstance(typ, str)
                and self._is_type(typ, dataclasses, KW_ONLY, self._is_kw_only)
            ):
                if self.KW_ONLY_seen:
                    raise TypeError(
                        f"{name!r} is KW_ONLY, but KW_ONLY "
                        "has already been specified"
                    )
                self.KW_ONLY_seen = True
                self.kw_only = True
            else:
                cls_fields.append(self._get_field(name, typ))

        for f in cls_fields:
            name: str = f.name  # type: ignore # name has been set
            self.fields[name] = f
            if hasattr(self.cls, name):
                # remove the field from the class
                # unlike dataclass, we don't actually
                # store any values in the class, they
                # are stored inside the IR node.
                delattr(self.cls, name)

    def _get_field(self, name: str, typ):
        """Return a Field object for this field name and type."""
        guess = self._guess_type(typ)
        default = getattr(self.cls, name, dataclasses.MISSING)
        if isinstance(default, Field):
            f = default
        # no default descriptor, create a new one
        elif is_subhint(typ, ir.SSAValue) or is_subhint(typ, tuple[ir.SSAValue, ...]):
            f: Field = (
                argument()
            )  # only argument can have default, others must use field specifiers
        elif typ is ir.ResultValue:
            raise ValueError(
                f"expect field specifiers for `ir.ResultValue` fields: {name}"
            )
        elif typ is ir.Region:
            raise ValueError(f"expect field specifiers for `ir.Region` fields: {name}")
        elif typ is ir.Block:
            raise ValueError(f"expect field specifiers for `ir.Block` fields: {name}")
        else:
            # could be a wrong SSAValue field, e.g list[SSAValue], try check args
            if typ_args := get_args(typ):
                if any(is_subhint(arg, ir.SSAValue) for arg in typ_args):
                    raise ValueError(
                        f"unsupported SSAValue field: {name},"
                        f" expect `SSAValue` or `tuple[SSAValue, ...]`, got {typ}"
                    )
            raise ValueError(f"expect field specifiers for attribute fields: {name}")

        f.name = name
        f.annotation = typ
        self._post_init_field(f, guess)
        return f

    def _post_init_field(self, f: Field, guess: type | None):
        if isinstance(f, ArgumentField) and is_subhint(guess, tuple[ir.SSAValue, ...]):
            f.group = True
        # try to narrow the type based on the guess
        elif isinstance(f, AttributeField):
            if guess and not is_subhint(
                guess, ir.Attribute
            ):  # not specified, and using python type
                if f.type is types.Any:  # not set or too generic
                    f.type = types.hint2type(guess)
                f.pytype = True

    @staticmethod
    def _is_kw_only(a_type):
        return a_type is KW_ONLY

    def _is_type(
        self,
        annotation: type | str,
        obj_module: ModuleType,
        obj_type: type,
        is_type_predicate: Callable[[type], bool],
    ):
        """Given a type annotation string, does it refer to `obj_type` in
        `obj_module`?  For example, when checking that annotation denotes a
        `ClassVar`, then `obj_module` is typing, and a_type is
        `typing.ClassVar`.

        Taken from dataclasses.py/is_type.
        """
        guess = self._guess_type(annotation)
        if guess is not None:
            return guess.__module__ is obj_module and is_type_predicate(guess)
        return False

    def _guess_type(self, annotation: type | str) -> type | None:
        """Guess the type/hint object from a string annotation."""
        if not isinstance(annotation, str):
            return annotation

        match = self._MODULE_IDENTIFIER_RE.match(annotation)
        ns = None
        if match:
            module_name = match.group(1)
            if not module_name:
                # No module name, assume the class's module did
                # "from dataclasses import InitVar".
                ns = sys.modules.get(self.cls.__module__).__dict__
            else:
                # Look up module_name in the class's module.
                if self.cls_module:
                    ns = self.cls_module.__dict__.get(module_name).__dict__
            if ns:
                return ns.get(match.group(2))
        return None
