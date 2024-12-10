from typing import Any

from kirin.decl.emit.init import BaseModifier
from kirin.exceptions import VerificationError

from ._create_fn import create_fn
from ._set_new_attribute import set_new_attribute


class EmitTypeCheck(BaseModifier):
    _VERIFICATION_ERROR = "_kirin_VerificationError"

    def emit_typecheck(self):
        typecheck_locals: dict[str, Any] = {
            self._VERIFICATION_ERROR: VerificationError,
        }
        body: list[str] = []
        for name, f in self.fields.args.items():
            if f.type is f.type.top():
                continue

            value_type = f"_args_{f.name}_type"
            typecheck_locals.update({value_type: f.type})
            if f.group:
                body.extend(
                    (
                        f"for v in {self._self_name}.{f.name}:",
                        *self._guard_ssa_type(f.name, value_type, indent=1),
                    )
                )
            else:
                body.extend(self._guard_ssa_type(f.name, value_type))

        for name, f in self.fields.results.items():
            if f.type is f.type.top():
                continue

            value_type = f"_results_{f.name}_type"
            typecheck_locals.update({value_type: f.type})
            body.extend(self._guard_ssa_type(name, value_type))

        for name in self.fields.blocks.keys():
            body.append(f"{self._self_name}.{name}.typecheck()")

        for name, f in self.fields.regions.items():
            body.append(f"{self._self_name}.{name}.typecheck()")

        # NOTE: we still need to generate this because it is abstract
        if not body:
            body.append("pass")

        set_new_attribute(
            self.cls,
            "typecheck",
            create_fn(
                name="_kirin_decl_typecheck",
                args=[self._self_name],
                body=body,
                globals=self.globals,
                locals=typecheck_locals,
                return_type=None,
            ),
        )

    def _guard_ssa_type(self, name, type, indent: int = 0):
        space = "  " * indent
        msg = f"'Invalid type for {name}, expected ' + repr({type}) + ', got ' + repr({self._self_name}.{name}.type)"
        return (
            space + f"if not {self._self_name}.{name}.type.is_subseteq({type}):",
            space + f"    raise {self._VERIFICATION_ERROR}({self._self_name}, {msg})",
        )
