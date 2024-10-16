from typing import Any

from kirin.decl.emit.init import BaseModifier

from ._create_fn import create_fn
from ._set_new_attribute import set_new_attribute


class EmitValidate(BaseModifier):

    def emit_validate(self):
        validate_locals: dict[str, Any] = {}
        body: list[str] = []
        for name, f in self.fields.args.items():
            if f.type.is_top():
                continue

            value_type = f"_args_{f.name}_type"
            validate_locals.update({value_type: f.type})
            if f.group:
                body.append(f"for v in {self._self_name}.{f.name}:")
                body.append(f"    assert v.type.is_subseteq({value_type})")
            else:
                body.append(
                    f"assert {self._self_name}.{f.name}.type.is_subseteq({value_type})"
                )

        for name, f in self.fields.results.items():
            if f.type.is_top():
                continue

            value_type = f"_results_{f.name}_type"
            validate_locals.update({value_type: f.type})
            body.append(
                f"assert {self._self_name}.{name}.type.is_subseteq({value_type})"
            )

        for name in self.fields.blocks.keys():
            body.append(f"{self._self_name}.{name}.validate()")

        for name, f in self.fields.regions.items():
            body.append(f"{self._self_name}.{name}.validate()")
            if not f.multi:
                body.append(f"assert len({self._self_name}.{name}.blocks) == 1")

        # NOTE: we still need to generate this because it is abstract
        if not body:
            body.append("pass")

        set_new_attribute(
            self.cls,
            "validate",
            create_fn(
                name="_validate",
                args=[self._self_name],
                body=body,
                globals=self.globals,
                locals=validate_locals,
                return_type=None,
            ),
        )
