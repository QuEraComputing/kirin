from typing import Any
from dataclasses import MISSING

from typing_extensions import Unpack

from kirin import ir, types
from kirin.decl import info
from kirin.decl.base import BaseModifier, StatementOptions

from ._create_fn import create_fn
from ._set_new_attribute import set_new_attribute


class _HAS_DEFAULT_FACTORY_CLASS:
    def __repr__(self):
        return "<factory>"


_HAS_DEFAULT_FACTORY = _HAS_DEFAULT_FACTORY_CLASS()


class EmitInit(BaseModifier):
    _POST_INIT_NAME = "__post_init__"
    _RESULT_VALUE_NAME = "_kirin_ResultValue"
    _KIRIN_PYATTR = "_kirin_PyAttr"
    _ATTR_FACTORY_PREFIX = "_kirin_attr_factory_"
    _SELF_CLASS = "_kirin_self_class"
    _KIRIN_STMT = "_kirin_Statement"

    def __init__(self, cls: type, **kwargs: Unpack[StatementOptions]) -> None:
        super().__init__(cls, **kwargs)
        from kirin.ir.attrs.py import PyAttr

        self._init_params: list[str] = []
        self._init_body: list[str] = []
        self._init_locals: dict[str, Any] = {}
        self.has_post_init = hasattr(self.cls, self._POST_INIT_NAME)
        self.globals.update(
            {
                "_PY_ANY": types.Any,
                self._KIRIN_STMT: ir.Statement,
                self._SELF_CLASS: self.cls,
                self._RESULT_VALUE_NAME: ir.ResultValue,
                self._KIRIN_PYATTR: PyAttr,
            }
        )

    @classmethod
    def _hint_name(cls, f: info.Field):
        return f"_hint_{f.name}"

    def emit_init(self):
        _args_groups = []
        for f in self.fields.args.values():
            if f.group:
                _args_groups.append(f.name)
        setattr(self.cls, "_arg_groups", frozenset(_args_groups))

        if "__init__" in self.cls.__dict__:
            return True

        self._init_locals.update(
            {self._hint_name(f): f.annotation for f in self.fields}
        )
        self._init_locals.update(
            {
                "MISSING": MISSING,
                "_HAS_DEFAULT_FACTORY": _HAS_DEFAULT_FACTORY,
            }
        )

        self._emit_params()
        self._emit_field_init(len(_args_groups) > 0)

        fn = create_fn(
            "__init__",
            self._init_params,
            self._init_body,
            globals=self.globals,
            locals=self._init_locals,
            return_type=None,
        )
        set_new_attribute(self.cls, "__init__", fn)

    def _emit_params(self):
        self._init_params.append(self._self_name)
        for f in self.fields.std_args.values():
            self._init_params.append(self._init_param_value(f))

        kw_params: list[str] = []
        for f in self.fields.kw_args.values():
            kw_params.append(self._init_param_value(f))

        for f in self.fields.attributes.values():
            if f.init:
                kw_params.append(self._init_param_value(f))

        for f in self.fields.regions.values():
            if f.init:
                kw_params.append(self._init_param_value(f))

        for f in self.fields.blocks.values():
            if f.init:
                kw_params.append(self._init_param_value(f))

        if kw_params:
            self._init_params.append("*")
            self._init_params.extend(kw_params)

    def _emit_field_init(self, has_args_groups: bool):
        self._init_body.append(
            (
                f"{self._KIRIN_STMT}.__init__({self._self_name},"
                f"args={self._arg_seq()},"
                f"regions={self._regions_seq()},"
                f"successors={self._blocks_seq()},"
                f"result_types={self._result_types_seq()},"
                f"attributes={self._attribute_seq()},"
                f"args_slice={self._args_slice(has_args_groups)}"
                ")"
            )
        )

        # TODO: support InitVar?
        if self.has_post_init:
            self._init_body.append(f"{self._self_name}.{self._POST_INIT_NAME}()")

    def _init_param_value(self, f: info.Field):
        if isinstance(f, info.AttributeField):  # only attribute can have defaults
            if f.default is MISSING and f.default_factory is None:
                default = ""
            elif f.default is not MISSING:
                default = f"={self._ATTR_FACTORY_PREFIX}{f.name}"
            elif f.default_factory is not None:
                default = "=_HAS_DEFAULT_FACTORY"
            else:
                raise ValueError("unexpected default")
            return f"{f.name}:{self._hint_name(f)}{default}"
        elif isinstance(f, info.ArgumentField):  # arguments are always required
            return f"{f.name}:{self._hint_name(f)}"
        elif isinstance(f, info.ResultField):
            raise ValueError("result fields are not allowed in __init__")
        elif isinstance(f, info.RegionField):
            return f"{f.name}:{self._hint_name(f)}=_HAS_DEFAULT_FACTORY"
        elif isinstance(f, info.BlockField):
            return f"{f.name}:{self._hint_name(f)}=_HAS_DEFAULT_FACTORY"
        else:  # the rest fields are all required
            raise ValueError(f"unexpected field type {type(f)}")

    def _arg_seq(self):
        args: list[str] = [self._field_param(f) for f in self.fields.args.values()]
        return self._tuple_str(args)

    def _field_param(self, f: info.ArgumentField):
        if f.group:
            return f"*{f.name}"
        else:
            return f"{f.name}"

    def _result_types_seq(self):
        result_types = [
            self._result_type_value(f) for f in self.fields.results.values()
        ]
        return self._tuple_str(result_types)

    def _result_type_value(self, f: info.ResultField):
        name = f"_result_type_{f.name}"
        self._init_locals[name] = f.type
        return name

    def _attribute_seq(self):
        attrs = ",".join(
            [
                f'"{name}": {self._attribute_value(f)}'
                for name, f in self.fields.attributes.items()
            ]
        )
        return "{" + attrs + "}"

    def _attribute_value(self, f: info.AttributeField):
        default_name = f"{self._ATTR_FACTORY_PREFIX}{f.name}"
        if f.default_factory is not None:
            value = self._field_with_default_factory(
                default_name, f.name, f.init, f.default_factory
            )
        else:
            # no default factory
            if f.init:
                if f.default is not MISSING:
                    self._init_locals[default_name] = f.default
                value: str = f.name  # type: ignore
            else:  # no default factory, no init, no default
                raise ValueError(
                    "attribute must have a default or default factory or be initialized (init=True)"
                )

        # declared via python type, optionally check if we can
        # convert the value to data.PyAttr
        if f.pytype:
            attr_type = f"_kirin_attr_type_{f.name}"
            self._init_locals[attr_type] = f.type
            value = (
                f"{self._KIRIN_PYATTR}({value}, {attr_type}) "
                f"if not isinstance({value}, {self._KIRIN_PYATTR}) "
                f"else {value}"
            )

        return value

    def _regions_seq(self):
        regions = [self._regions_value(f) for f in self.fields.regions.values()]
        return self._tuple_str(regions)

    def _regions_value(self, f: info.RegionField):
        return self._field_with_default_factory(
            f"_kirin_region_{f.name}", f.name, f.init, f.default_factory
        )

    def _blocks_seq(self):
        blocks = [self._blocks_value(f) for f in self.fields.blocks.values()]
        return self._tuple_str(blocks)

    def _blocks_value(self, f: info.BlockField):
        return self._field_with_default_factory(
            f"_kirin_block_{f.name}", f.name, f.init, f.default_factory
        )

    def _args_slice(self, has_args_groups: bool):
        if not has_args_groups:
            return (
                "{"
                + ", ".join(
                    [
                        f"'{f.name}': {index}"
                        for index, f in enumerate(self.fields.args.values())
                    ]
                )
                + "}"
            )

        # NOTE: SSAValue fields do not have default/default_factory
        # so we can just count the input arguments
        ret: list[str] = []
        self._init_body.append("__args_slice_count = 0")
        for f in self.fields.args.values():
            if f.group:
                self._init_body.append(
                    f"__{f.name}_slice = slice(__args_slice_count, __args_slice_count + len({f.name}))"
                )
                self._init_body.append(f"__args_slice_count += len({f.name})")
            else:
                self._init_body.append(f"__{f.name}_slice = __args_slice_count")
                self._init_body.append("__args_slice_count += 1")
            ret.append(f'"{f.name}": __{f.name}_slice')
        return "{" + ",".join(ret) + "}"

    def _field_with_default_factory(
        self, factory_name: str, name: str | None, init: bool, default_factory
    ):
        # block always has default_factory
        if init:
            self._init_locals[factory_name] = default_factory
            value = (
                f"{factory_name}() "
                f"if {name} is _HAS_DEFAULT_FACTORY "
                f"else {name}"
            )
        else:
            self._init_locals[factory_name] = default_factory
            value = f"{factory_name}()"
        return value

    @staticmethod
    def _tuple_str(seq: list[str]):
        if not seq:
            return "()"
        return f"({', '.join(seq)}, )"
