from abc import abstractmethod
from types import GenericAlias
from typing import Any, Callable, Optional
from functools import cached_property
from dataclasses import MISSING, field, dataclass

from kirin import types
from kirin.ir import Block, Region, Attribute


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
    type: types.TypeAttribute
    pytype: bool = False
    "if `True`, annotation is a python type hint instead of `TypeAttribute`"

    def has_no_default(self):
        return self.default is MISSING and self.default_factory is None


def attribute(
    type: types.TypeAttribute = types.Any,
    *,
    init: bool = True,
    repr: bool = True,
    default: Any = MISSING,
    default_factory: Optional[Callable[[], Any]] = None,
    kw_only: bool = True,
    alias: Optional[str] = None,
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
    )


@dataclass
class ArgumentField(Field):
    type: types.TypeAttribute
    """type of the argument, will be used in validation.
    """
    print: bool = True
    """if `True`, this argument name is printed in the signature.
    """
    group: bool = False  # NOTE: this cannot be set by user
    """if `True`, this argument is annotated with Tuple[SSAValue, ...]
    """

    def has_no_default(self):
        return True


# NOTE: argument must appear in init and repr
def argument(
    type: types.TypeAttribute = types.Any,
    *,
    print: bool = True,
    kw_only: bool = False,
    alias: Optional[str] = None,
) -> Any:
    """Field specifier for arguments.

    Args:
        type(TypeAttribute): type of the argument, will be used in validation.
        print(bool): if `True`, this argument name is printed in the signature.
        kw_only(bool): if `True`, this argument is keyword-only.
        alias(Optional[str]): an alias for the argument name in the `__init__` method.
    """
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
    type: types.TypeAttribute = field(default_factory=types.AnyType)

    def has_no_default(self):
        return True


def result(
    type: types.TypeAttribute = types.Any,
    *,
    # NOTE: init is false, use other hooks to set custom results
    # or just mutate the statement after creation
    init: bool = False,
    repr: bool = True,
    kw_only: bool = True,
    alias: Optional[str] = None,
) -> Any:
    """Field specifier for results.

    Args:
        type(TypeAttribute): type of the result.
        init(bool): if `True`, this result field is included in the `__init__` method.
        repr(bool): if `True`, this result field is included in the `__repr__` and pretty printing.
        kw_only(bool): if `True`, this result field is keyword-only.
        alias(Optional[str]): an alias for the result field name in the `__init__` method.
    """
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
    """Field specifier for regions.

    Args:
        init(bool): if `True`, this region field is included in the `__init__` method.
        repr(bool): if `True`, this region field is included in the `__repr__` and pretty printing.
        kw_only(bool): if `True`, this region field is keyword-only.
        alias(Optional[str]): an alias for the region field name in the `__init__` method.
        multi(bool): if `True`, this region can contain multiple blocks.
        default_factory(Callable[[], Region]): a factory function to create a default region.
    """
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
    """Field specifier for blocks.

    Args:
        init(bool): if `True`, this block field is included in the `__init__` method.
        repr(bool): if `True`, this block field is included in the `__repr__` and pretty printing.
        kw_only(bool): if `True`, this block field is keyword-only.
        alias(Optional[str]): an alias for the block field name in the `__init__` method.
        default_factory(Callable[[], Block]): a factory function to create a default block.
    """
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
    """standard arguments of the statement."""
    kw_args: dict[str, ArgumentField] = field(default_factory=dict)
    """keyword-only arguments of the statement."""
    results: dict[str, ResultField] = field(default_factory=dict)
    """results of the statement."""
    regions: dict[str, RegionField] = field(default_factory=dict)
    """regions of the statement."""
    blocks: dict[str, BlockField] = field(default_factory=dict)
    """blocks of the statement."""
    attributes: dict[str, AttributeField] = field(default_factory=dict)
    """attributes of the statement."""

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
        """iterable of all argument fields."""
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

    def __len__(self):
        return (
            len(self.args)
            + len(self.results)
            + len(self.regions)
            + len(self.blocks)
            + len(self.attributes)
        )

    @cached_property
    def attr_or_props(self):
        return set(self.attributes.keys())

    @cached_property
    def required_names(self):
        """set of all fields that do not have a default value."""
        return set(
            list(self.args.keys())
            + [name for name, f in self.attributes.items() if f.has_no_default()]
            + [name for name, f in self.blocks.items() if f.has_no_default()]
            + [name for name, f in self.regions.items() if f.has_no_default()]
        )

    @cached_property
    def group_arg_names(self):
        return set([name for name, f in self.args.items() if f.group])
