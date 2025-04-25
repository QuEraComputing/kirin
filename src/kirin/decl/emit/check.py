from typing import Any

from kirin.ir.exception import ValidationError
from kirin.decl.emit.init import BaseModifier

from ._create_fn import create_fn
from ._set_new_attribute import set_new_attribute


class EmitCheck(BaseModifier):
    _VERIFICATION_ERROR = "_kirin_ValidationError"

    def emit_check(self):
        check_locals: dict[str, Any] = {
            self._VERIFICATION_ERROR: ValidationError,
        }
        body: list[str] = []
        for name in self.fields.blocks.keys():
            body.append(f"{self._self_name}.{name}.verify()")

        for name, f in self.fields.regions.items():
            body.append(f"{self._self_name}.{name}.verify()")
            if not f.multi:
                body.append(f"if len({self._self_name}.{name}.blocks) != 1:")
                body.append(
                    f"    raise {self._VERIFICATION_ERROR}({self._self_name},"
                    f" 'Invalid number of blocks for {name}')"
                )

        if (traits := getattr(self.cls, "traits", None)) is not None:
            for trait in traits:
                trait_obj = f"_kirin_check_trait_{trait.__class__.__name__}"
                check_locals.update({trait_obj: trait})
                body.append(f"{trait_obj}.verify({self._self_name})")

        # NOTE: we still need to generate this because it is abstract
        if not body:
            body.append("pass")

        set_new_attribute(
            self.cls,
            "check",
            create_fn(
                name="_kirin_decl_check",
                args=[self._self_name],
                body=body,
                globals=self.globals,
                locals=check_locals,
                return_type=None,
            ),
        )
