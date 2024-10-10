from abc import abstractmethod
from dataclasses import MISSING, dataclass, field
from functools import cached_property
from types import GenericAlias
from typing import Any, Callable, Optional

from kirin.dialects.py.types import Any as PyAny, PyAnyType
from kirin.ir import Attribute, Block, Region, TypeAttribute


@dataclass
class Field:
    name: Optional[str] = field(default=None, init=False)
    annotation: Any = field(default=None, init=False)
    kw_only: bool
    alias: Optional[str]

    __class_getitem__ = classmethod(GenericAlias)

    @abstractmethod
    def has_no_default(self) -> bool: ...


@dataclass
class AttributeField(Field):
    default: Any
    init: bool
    repr: bool
    default_factory: Optional[Callable[[], Attribute]]
    type: TypeAttribute
    property: bool
    pytype: bool = False
    "if `True`, annotation is a python type hint instead of `TypeAttribute`"

    def has_no_default(self):
        return self.default is MISSING and self.default_factory is None


def attribute(
    type: TypeAttribute = PyAny,
    *,
    init: bool = True,
    repr: bool = True,
    default: Any = MISSING,
    default_factory: Optional[Callable[[], Any]] = None,
    kw_only: bool = True,
    alias: Optional[str] = None,
    property: bool = False,
) -> Any:
    if kw_only is False:
        raise TypeError("attribute fields must be keyword-only")

    return AttributeField(
        type=type,
        init=init,
        repr=repr,
        default=default,
        default_factory=default_factory,
        kw_only=kw_only,
        alias=alias,
        property=property,
    )


@dataclass
class ArgumentField(Field):
    type: TypeAttribute
    print: bool = True
    group: bool = False  # NOTE: this cannot be set by user

    def has_no_default(self):
        return True


# NOTE: argument must appear in init and repr
def argument(
    type: TypeAttribute = PyAny,
    *,
    print: bool = True,
    kw_only: bool = False,
    alias: Optional[str] = None,
) -> Any:
    return ArgumentField(
        type=type,
        print=print,
        kw_only=kw_only,
        alias=alias,
    )


@dataclass
class ResultField(Field):
    init: bool
    repr: bool
    type: TypeAttribute = field(default_factory=PyAnyType)

    def has_no_default(self):
        return True


def result(
    type: TypeAttribute = PyAny,
    *,
    # NOTE: init is false, use other hooks to set custom results
    # or just mutate the statement after creation
    init: bool = False,
    repr: bool = True,
    kw_only: bool = True,
    alias: Optional[str] = None,
) -> Any:
    if kw_only is False:  # for linting
        raise TypeError("result fields must be keyword-only")

    if init is True:
        raise TypeError("result fields cannot appear in __init__")

    return ResultField(
        type=type,
        init=init,
        repr=repr,
        kw_only=kw_only,
        alias=alias,
    )


@dataclass
class RegionField(Field):
    init: bool
    repr: bool
    multi: bool
    default_factory: Callable[[], Region]

    def has_no_default(self):
        return False


def region(
    *,
    init: bool = True,  # so we can use the default_factory
    repr: bool = True,
    kw_only: bool = True,
    alias: Optional[str] = None,
    multi: bool = False,
    default_factory: Callable[[], Region] = Region,
) -> Any:
    if kw_only is False:
        raise TypeError("region fields must be keyword-only")

    return RegionField(
        init=init,
        repr=repr,
        kw_only=kw_only,
        alias=alias,
        multi=multi,
        default_factory=default_factory,
    )


@dataclass
class BlockField(Field):
    init: bool
    repr: bool
    default_factory: Callable[[], Block]

    def has_no_default(self):
        return False


def block(
    *,
    init: bool = True,
    repr: bool = True,
    kw_only: bool = True,
    alias: Optional[str] = None,
    default_factory: Callable[[], Block] = Block,
) -> Any:
    if kw_only is False:
        raise TypeError("block fields must be keyword-only")

    return BlockField(
        init=init,
        repr=repr,
        kw_only=kw_only,
        alias=alias,
        default_factory=default_factory,
    )


@dataclass
class StatementFields:
    std_args: dict[str, ArgumentField] = field(default_factory=dict)
    kw_args: dict[str, ArgumentField] = field(default_factory=dict)
    results: dict[str, ResultField] = field(default_factory=dict)
    regions: dict[str, RegionField] = field(default_factory=dict)
    blocks: dict[str, BlockField] = field(default_factory=dict)
    attributes: dict[str, AttributeField] = field(default_factory=dict)
    properties: dict[str, AttributeField] = field(default_factory=dict)

    class Args:
        def __init__(self, fields: "StatementFields"):
            self.fields = fields

        def __len__(self):
            return len(self.fields.std_args) + len(self.fields.kw_args)

        def __getitem__(self, name):
            if (value := self.fields.std_args.get(name)) is not None:
                return value
            elif (value := self.fields.kw_args.get(name)) is not None:
                return value
            raise KeyError(name)

        def __setitem__(self, name: str, value: ArgumentField):
            if value.kw_only:
                self.fields.kw_args[name] = value
            else:
                self.fields.std_args[name] = value

        def __contains__(self, name):
            return name in self.fields.std_args or name in self.fields.kw_args

        def values(self):
            yield from self.fields.std_args.values()
            yield from self.fields.kw_args.values()

        def items(self):
            yield from self.fields.std_args.items()
            yield from self.fields.kw_args.items()

        def keys(self):
            yield from self.fields.std_args.keys()
            yield from self.fields.kw_args.keys()

    @property
    def args(self):
        return self.Args(self)

    @classmethod
    def from_fields(cls, fields: dict[str, Field]):
        ret = cls()
        for name, f in fields.items():
            ret[name] = f
        return ret

    def __contains__(self, name):
        return (
            name in self.args
            or name in self.results
            or name in self.regions
            or name in self.blocks
            or name in self.attributes
            or name in self.properties
        )

    def __setitem__(self, name, value):
        if isinstance(value, ArgumentField):
            self.args[name] = value
        elif isinstance(value, ResultField):
            self.results[name] = value
        elif isinstance(value, RegionField):
            self.regions[name] = value
        elif isinstance(value, BlockField):
            self.blocks[name] = value
        elif isinstance(value, AttributeField):
            if value.property:
                self.properties[name] = value
            else:
                self.attributes[name] = value
        else:
            raise TypeError(f"unknown field type {value}")

    def __iter__(self):
        yield from self.args.values()
        yield from self.kw_args.values()
        yield from self.results.values()
        yield from self.regions.values()
        yield from self.blocks.values()
        yield from self.attributes.values()
        yield from self.properties.values()

    def __len__(self):
        return (
            len(self.args)
            + len(self.results)
            + len(self.regions)
            + len(self.blocks)
            + len(self.attributes)
            + len(self.properties)
        )

    @cached_property
    def attr_or_props(self):
        return set(list(self.attributes.keys()) + list(self.properties.keys()))

    @cached_property
    def required_names(self):
        return set(
            list(self.args.keys())
            + [name for name, f in self.attributes.items() if f.has_no_default()]
            + [name for name, f in self.properties.items() if f.has_no_default()]
            + [name for name, f in self.blocks.items() if f.has_no_default()]
            + [name for name, f in self.regions.items() if f.has_no_default()]
        )

    @cached_property
    def group_arg_names(self):
        return set([name for name, f in self.args.items() if f.group])
