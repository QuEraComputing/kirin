import inspect
import textwrap

from beartype.door import is_subhint

from kirin import ir

from .base import BaseModifier
from .info import Field, ResultField, ArgumentField


class Verify(BaseModifier):
    RESERVED_NAMES = {
        "name",
        "traits",
        "dialect",
        "args",
        "parent",
        "parent_node",
        "parent_region",
        "parent_block",
        "next_stmt",
        "prev_stmt",
        "results",
        "regions",
        "successors",
        "attributes",
    }

    def verify_mro(self):
        if not issubclass(self.cls, ir.Statement):
            raise TypeError(
                f"expected {self.cls.__name__} to be a subclass of ir.Statement"
            )

    def verify_fields(self):
        # Do we have any Field members that don't also have annotations?
        cls_annotations = inspect.get_annotations(self.cls)
        for name, value in self.cls.__dict__.items():
            if isinstance(value, Field) and name not in cls_annotations:
                raise ValueError(f"{name!r} is a field but has no type annotation")

        for f in self.fields:
            if f.name in self.RESERVED_NAMES:
                raise ValueError(f"{f.name!r} is a reserved name")

            if isinstance(f, ArgumentField):
                if not (
                    is_subhint(f.annotation, ir.SSAValue)
                    or is_subhint(f.annotation, tuple[ir.SSAValue, ...])
                ):
                    raise ValueError(
                        f"{f.name!r} is an argument field but has an invalid type annotation"
                    )

                if is_subhint(f.annotation, ir.ResultValue):
                    raise ValueError(
                        textwrap.dedent(
                            f"{f.name!r} is an argument field but has an invalid type annotation {f.annotation}"
                            " (did you mean to use `info.result` instead?)"
                        )
                    )

            if isinstance(f, ResultField) and not is_subhint(
                f.annotation, ir.ResultValue
            ):
                raise ValueError(
                    f"{f.name!r} is a result field but has an invalid type annotation"
                )
