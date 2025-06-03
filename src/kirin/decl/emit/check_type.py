from typing import Any

from kirin.ir.exception import ValidationError
from kirin.decl.emit.init import BaseModifier

from ._create_fn import create_fn
from ._set_new_attribute import set_new_attribute


class EmitCheckType(BaseModifier):
    _VERIFICATION_ERROR = "_kirin_ValidationError"

    def emit_check_type(self):
        check_type_locals: dict[str, Any] = {
            self._VERIFICATION_ERROR: ValidationError,
        }
        body: list[str] = []
        for name, f in self.fields.args.items():
            if f.type is f.type.top():
                continue

            value_type = f"_args_{f.name}_type"
            check_type_locals.update({value_type: f.type})
            if f.group:
                body.extend(
                    (
                        f"for v in {self._self_name}.{f.name}:",
                        *self._guard_ssa_type("v", f.name, value_type, indent=1),
                    )
                )
            else:
                body.extend(
                    self._guard_ssa_type(
                        f"{self._self_name}.{name}", f.name, value_type
                    )
                )

        for name, f in self.fields.results.items():
            if f.type is f.type.top():
                continue

            value_type = f"_results_{f.name}_type"
            check_type_locals.update({value_type: f.type})
            body.extend(
                self._guard_ssa_type(f"{self._self_name}.{name}", name, value_type)
            )

        for name, f in self.fields.regions.items():
            body.append(f"{self._self_name}.{name}.verify_type()")

        # NOTE: we still need to generate this because it is abstract
        if not body:
            body.append("pass")

        set_new_attribute(
            self.cls,
            "check_type",
            create_fn(
                name="_kirin_decl_check_type",
                args=[self._self_name],
                body=body,
                globals=self.globals,
                locals=check_type_locals,
                return_type=None,
            ),
        )

    def _guard_ssa_type(self, ssa, name, type, indent: int = 0):
        space = "  " * indent
        msg = f"'Invalid type for {name}, expected ' + repr({type}) + ', got ' + repr({ssa}.type)"
        return (
            space + f"if not {ssa}.type.is_subseteq({type}):",
            space + f"    raise {self._VERIFICATION_ERROR}({self._self_name}, {msg})",
        )
