from kirin import ir
from kirin.decl import info
from kirin.decl.emit.init import BaseModifier

from ._create_fn import create_fn
from ._set_new_attribute import set_new_attribute


class EmitProperty(BaseModifier):
    _KIRIN_PYATTR = "_kirin_PyAttr"

    def emit_property(self):
        for name, f in self.fields.args.items():
            getter, setter = self._emit_arg_property(f)
            set_new_attribute(self.cls, name, property(getter, setter))

        for name, f in self.fields.kw_args.items():
            getter, setter = self._emit_arg_property(f)
            set_new_attribute(self.cls, name, property(getter, setter))

        for i, (name, f) in enumerate(self.fields.results.items()):
            getter, setter = self._emit_result_property(i, f)
            set_new_attribute(self.cls, name, property(getter, setter))

        for name, f in self.fields.attributes.items():
            getter, setter = self._emit_attribute_property(f)
            set_new_attribute(self.cls, name, property(getter, setter))

        for i, (name, f) in enumerate(self.fields.blocks.items()):
            getter, setter = self._emit_successor_property(i, f)
            set_new_attribute(self.cls, name, property(getter, setter))

        for i, (name, f) in enumerate(self.fields.regions.items()):
            getter, setter = self._emit_region_property(i, f)
            set_new_attribute(self.cls, name, property(getter, setter))

    def _emit_arg_property(self, f: info.ArgumentField):
        getter = create_fn(
            f"_get_{f.name}",
            args=[self._self_name],
            body=[
                f'return {self._self_name}._args[{self._self_name}._name_args_slice["{f.name}"]]'
            ],
            globals=self.globals,
            return_type=tuple[ir.SSAValue, ...] if f.group else ir.SSAValue,
        )

        if f.group:
            return getter, self._emit_arg_property_group_setter(f)
        return getter, self._emit_arg_property_std_setter(f)

    def _emit_arg_property_std_setter(self, f: info.ArgumentField):
        return create_fn(
            f"_set_{f.name}",
            args=[self._self_name, "value: _value_hint"],
            body=[
                f"s = {self._self_name}._name_args_slice['{f.name}']",
                f"old = {self._self_name}.args[s]",
                f"old.remove_use(Use({self._self_name}, s))",
                f"value.add_use(Use({self._self_name}, s))",
                "self.args = (*self.args[:s], value, *self.args[s + 1:])",
            ],
            globals=self.globals,
            locals={"_value_hint": ir.SSAValue},
            return_type=None,
        )

    def _emit_arg_property_group_setter(self, f: info.ArgumentField):
        return create_fn(
            f"_set_{f.name}",
            args=[self._self_name, "value: _value_hint"],
            body=[
                f"s = {self._self_name}._name_args_slice['{f.name}']",
                "start, stop = s.start, s.stop",
                f"old = {self._self_name}.args[s]",
                "a_range = range(start, stop, s.step or 1)",
                "for i, arg in zip(a_range, old):",
                f"    arg.remove_use(Use({self._self_name}, i))",
                "for i, arg in zip(a_range, value):",
                f"    arg.add_use(Use({self._self_name}, i))",
                f"{self._self_name}.args = (*{self._self_name}.args[:start], *value, *{self._self_name}.args[stop + 1:])",
            ],
            globals=self.globals,
            locals={"_value_hint": tuple[ir.SSAValue, ...]},
            return_type=None,
        )

    def _emit_result_property(self, index: int, f: info.ResultField):
        getter = create_fn(
            f"_get_{f.name}",
            args=[self._self_name],
            body=[f"return {self._self_name}._results[{index}]"],
            globals=self.globals,
            return_type=ir.ResultValue,
        )

        # well you cannot delete what already happened, can you?
        setter = create_fn(
            f"_set_{f.name}",
            args=[self._self_name, "value: _value_hint"],
            body=[f"raise AttributeError('result property {f.name} is read-only')"],
            globals=self.globals,
            locals={"_value_hint": ir.ResultValue},
            return_type=None,
        )

        return getter, setter

    def _emit_attribute_property(self, f: info.AttributeField):
        from kirin.ir.attrs.py import PyAttr

        storage = "attributes"
        attr = f"{self._self_name}.{storage}['{f.name}']"
        getter = create_fn(
            f"_get_{f.name}",
            args=[self._self_name],
            body=[f"return {attr}.data" if f.pytype else f"return {attr}"],
            globals=self.globals,
            return_type=f.annotation,
        )

        setter = create_fn(
            f"_set_{f.name}",
            args=[self._self_name, "value: _value_hint"],
            body=[
                (
                    f"{attr} = value if isinstance(value, {self._KIRIN_PYATTR}) else {self._KIRIN_PYATTR}(value)"
                    if f.pytype
                    else f"{attr} = value"
                )
            ],
            globals=self.globals,
            locals={"_value_hint": PyAttr if f.pytype else f.annotation},
            return_type=None,
        )

        return getter, setter

    def _emit_successor_property(self, index: int, f: info.BlockField):
        getter = create_fn(
            f"_get_{f.name}",
            args=[self._self_name],
            body=[f"return {self._self_name}.successors[{index}]"],
            globals=self.globals,
            return_type=ir.Block,
        )

        setter = create_fn(
            f"_set_{f.name}",
            args=[self._self_name, "value: _value_hint"],
            body=[f"{self._self_name}.successors[{index}] = value"],
            globals=self.globals,
            locals={"_value_hint": ir.Block},
            return_type=None,
        )

        return getter, setter

    def _emit_region_property(self, index: int, f: info.RegionField):
        getter = create_fn(
            f"_get_{f.name}",
            args=[self._self_name],
            body=[f"return {self._self_name}.regions[{index}]"],
            globals=self.globals,
            return_type=ir.Region,
        )

        setter = create_fn(
            f"_set_{f.name}",
            args=[self._self_name, "value: _value_hint"],
            body=[
                f"old = {self._self_name}.regions[{index}]",
                "old.parent = None",
                f"value.parent = {self._self_name}",
                f"{self._self_name}.regions[{index}] = value",
            ],
            globals=self.globals,
            locals={"_value_hint": ir.Region},
            return_type=None,
        )

        return getter, setter
