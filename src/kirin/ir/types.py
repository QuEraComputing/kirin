import typing
from abc import abstractmethod
from dataclasses import dataclass

from beartype.door import TupleVariableTypeHint  # type: ignore
from beartype.door import TypeHint, ClassTypeHint, TypeVarTypeHint
from typing_extensions import Never

from kirin.print import Printer
from kirin.lattice import (
    UnionMeta,
    LatticeMeta,
    SingletonMeta,
    BoundedLattice,
    IsSubsetEqMixin,
    SimpleMeetMixin,
)
from kirin.ir.attrs import Attribute, AttributeMeta
from kirin.ir._types import _TypeAttribute


class TypeAttributeMeta(AttributeMeta, LatticeMeta):
    """Metaclass for type attributes."""

    pass


class SingletonTypeMeta(TypeAttributeMeta, SingletonMeta):
    """Metaclass for singleton type attributes.

    Singleton type attributes are attributes that have only one instance.

    Examples:
    - `AnyType`
    - `BottomType`
    """

    pass


class UnionTypeMeta(TypeAttributeMeta, UnionMeta):
    pass


@dataclass
class TypeAttribute(
    _TypeAttribute,
    SimpleMeetMixin["TypeAttribute"],
    IsSubsetEqMixin["TypeAttribute"],
    BoundedLattice["TypeAttribute"],
    metaclass=TypeAttributeMeta,
):

    @classmethod
    def top(cls) -> "TypeAttribute":
        return AnyType()

    @classmethod
    def bottom(cls) -> "TypeAttribute":
        return BottomType()

    def join(self, other: "TypeAttribute") -> "TypeAttribute":
        if self.is_subseteq(other):
            return other
        elif other.is_subseteq(self):
            return self
        elif isinstance(other, TypeAttribute):
            return Union(self, other)
        return AnyType()  # don't know how to join

    def is_subseteq_Hinted(self, other: "Hinted") -> bool:
        return self.is_subseteq(other.type)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self, prefix="!")

    def __or__(self, other: "TypeAttribute"):
        return self.join(other)

    def __eq__(self, value: object) -> bool:
        return isinstance(value, TypeAttribute) and self.is_equal(value)

    @abstractmethod
    def __hash__(self) -> int: ...


@typing.final
@dataclass
class AnyType(TypeAttribute, metaclass=SingletonTypeMeta):
    name = "Any"

    def is_subseteq_TypeVar(self, other: "TypeVar") -> bool:
        return self.is_subseteq(other.bound)

    def __hash__(self) -> int:
        return id(self)


@typing.final
@dataclass
class BottomType(TypeAttribute, metaclass=SingletonTypeMeta):
    name = "Bottom"

    def is_subseteq(self, other: TypeAttribute) -> bool:
        if isinstance(other, TypeVar):
            return self.is_subseteq(other.bound)
        return True

    def __hash__(self) -> int:
        return id(self)


class PyClassMeta(TypeAttributeMeta):

    def __init__(self, *args, **kwargs):
        super(PyClassMeta, self).__init__(*args, **kwargs)
        self._cache = {}

    def __call__(self, typ):
        if typ is typing.Any:
            return AnyType()
        elif typ is typing.NoReturn or typ is Never:
            return BottomType()
        elif typ is typing.Tuple:
            typ = tuple
        elif typ is typing.List:
            typ = list
        elif isinstance(typ, TypeVar):
            return hint2type(typ)
        elif isinstance(typ, type) and typ in self._cache:
            return self._cache[typ]

        instance = super(PyClassMeta, self).__call__(typ)
        self._cache[typ] = instance
        return instance


PyClassType = typing.TypeVar("PyClassType")


@typing.final
@dataclass
class PyClass(TypeAttribute, typing.Generic[PyClassType], metaclass=PyClassMeta):
    name = "PyClass"
    typ: type[PyClassType]

    def __init__(self, typ: type[PyClassType]) -> None:
        self.typ = typ

    def is_subseteq_PyClass(self, other: "PyClass") -> bool:
        return issubclass(self.typ, other.typ)

    def is_subseteq_Union(self, other: "Union") -> bool:
        return any(self.is_subseteq(t) for t in other.types)

    def is_subseteq_Generic(self, other: "Generic") -> bool:
        # NOTE: subclass without generics is just generic with all any parameters
        Any = AnyType()
        return (
            self.is_subseteq(other.body)
            and all(Any.is_subseteq(bound) for bound in other.vars)
            and (other.vararg is None or Any.is_subseteq(other.vararg.typ))
        )

    def is_subseteq_TypeVar(self, other: "TypeVar") -> bool:
        return self.is_subseteq(other.bound)

    def __hash__(self) -> int:
        return hash((PyClass, self.typ))

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print("!py.", self.typ.__name__)


class LiteralMeta(TypeAttributeMeta):

    def __init__(self, *args, **kwargs):
        super(LiteralMeta, self).__init__(*args, **kwargs)
        self._cache = {}

    def __call__(self, data):
        if isinstance(data, Attribute):
            return data
        elif data in self._cache:
            return self._cache[data]

        instance = super(LiteralMeta, self).__call__(data)
        self._cache[data] = instance
        return instance


LiteralType = typing.TypeVar("LiteralType")


@typing.final
@dataclass
class Literal(TypeAttribute, typing.Generic[LiteralType], metaclass=LiteralMeta):
    name = "Literal"
    data: LiteralType

    def is_equal(self, other: TypeAttribute) -> bool:
        return self is other

    def is_subseteq_TypeVar(self, other: "TypeVar") -> bool:
        return self.is_subseteq(other.bound)

    def is_subseteq_Union(self, other: "Union") -> bool:
        return any(self.is_subseteq(t) for t in other.types)

    def is_subseteq_fallback(self, other: TypeAttribute) -> bool:
        return self.is_equal(other)

    def __hash__(self) -> int:
        return hash((Literal, self.data))

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(repr(self.data))


@typing.final
@dataclass
class Union(TypeAttribute, metaclass=UnionTypeMeta):
    name = "Union"
    types: frozenset[TypeAttribute]

    def __init__(
        self,
        typ_or_set: TypeAttribute | typing.Iterable[TypeAttribute],
        *typs: TypeAttribute,
    ):
        if isinstance(typ_or_set, TypeAttribute):
            params: typing.Iterable[TypeAttribute] = (typ_or_set, *typs)
        else:
            params = typ_or_set
            assert not typs, "Cannot pass multiple arguments when passing a set"

        types: frozenset[TypeAttribute] = frozenset()
        for typ in params:
            if isinstance(typ, Union):
                types = types.union(typ.types)
            else:
                types = types.union({typ})
        self.types = types

    def is_equal(self, other: TypeAttribute) -> bool:
        return isinstance(other, Union) and self.types == other.types

    def is_subseteq_fallback(self, other: TypeAttribute) -> bool:
        return all(t.is_subseteq(other) for t in self.types)

    def join(self, other: TypeAttribute) -> TypeAttribute:
        if self.is_subseteq(other):
            return other
        elif other.is_subseteq(self):
            return self
        elif isinstance(other, Union):
            return Union(self.types | other.types)
        elif isinstance(other, TypeAttribute):
            return Union(self.types | {other})
        return BottomType()

    def meet(self, other: TypeAttribute) -> TypeAttribute:
        if self.is_subseteq(other):
            return self
        elif other.is_subseteq(self):
            return other
        elif isinstance(other, Union):
            return Union(self.types & other.types)
        elif isinstance(other, TypeAttribute):
            return Union(self.types & {other})
        return BottomType()

    def __hash__(self) -> int:
        return hash((Union, self.types))

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self, prefix="!")
        printer.print_seq(self.types, delim=", ", prefix="[", suffix="]")


@typing.final
@dataclass
class TypeVar(TypeAttribute):
    name = "TypeVar"
    varname: str
    bound: TypeAttribute

    def __init__(self, name: str, bound: TypeAttribute | None = None):
        self.varname = name
        self.bound = bound or AnyType()

    def is_equal(self, other: TypeAttribute) -> bool:
        return (
            isinstance(other, TypeVar)
            and self.varname == other.varname
            and self.bound.is_equal(other.bound)
        )

    def is_subseteq_TypeVar(self, other: "TypeVar") -> bool:
        return self.bound.is_subseteq(other.bound)

    def is_subseteq_Union(self, other: Union) -> bool:
        return any(self.is_subseteq(t) for t in other.types)

    def is_subseteq_fallback(self, other: TypeAttribute) -> bool:
        return self.bound.is_subseteq(other)

    def __hash__(self) -> int:
        return hash((TypeVar, self.varname, self.bound))

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(f"~{self.varname}")
        if self.bound is not self.bound.top():
            printer.plain_print(" : ")
            printer.print(self.bound)


@typing.final
@dataclass
class Vararg(Attribute):
    name = "Vararg"
    typ: TypeAttribute

    def __hash__(self) -> int:
        return hash((Vararg, self.typ))

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print("*")
        printer.print(self.typ)


TypeVarValue: typing.TypeAlias = TypeAttribute | Vararg | list
TypeOrVararg: typing.TypeAlias = TypeAttribute | Vararg


@typing.final
@dataclass
class Generic(TypeAttribute, typing.Generic[PyClassType]):
    name = "Generic"
    body: PyClass[PyClassType]
    vars: tuple[TypeAttribute, ...]
    vararg: Vararg | None = None

    def __init__(
        self,
        body: type[PyClassType] | PyClass[PyClassType],
        *vars: TypeAttribute | list | Vararg,
    ):
        if isinstance(body, PyClass):
            self.body = body
        else:
            self.body = PyClass(body)
        self.vars, self.vararg = _split_type_args(vars)

    def is_subseteq_Literal(self, other: Literal) -> bool:
        return False

    def is_subseteq_PyClass(self, other: PyClass) -> bool:
        return self.body.is_subseteq(other)

    def is_subseteq_Union(self, other: Union) -> bool:
        return any(self.is_subseteq(t) for t in other.types)

    def is_subseteq_TypeVar(self, other: TypeVar) -> bool:
        return self.body.is_subseteq(other.bound)

    def is_subseteq_Generic(self, other: "Generic") -> bool:
        if other.vararg is None:
            return (
                self.body.is_subseteq(other.body)
                and len(self.vars) == len(other.vars)
                and all(v.is_subseteq(o) for v, o in zip(self.vars, other.vars))
            )
        else:
            return (
                self.body.is_subseteq(other.body)
                and len(self.vars) >= len(other.vars)
                and all(v.is_subseteq(o) for v, o in zip(self.vars, other.vars))
                and all(
                    v.is_subseteq(other.vararg.typ)
                    for v in self.vars[len(other.vars) :]
                )
                and (
                    self.vararg is None or self.vararg.typ.is_subseteq(other.vararg.typ)
                )
            )

    def __hash__(self) -> int:
        return hash((Generic, self.body, self.vars, self.vararg))

    def print_impl(self, printer: Printer) -> None:
        printer.print(self.body)
        printer.plain_print("[")
        if self.vars:
            printer.print_seq(self.vars)
        if self.vararg is not None:
            if self.vars:
                printer.plain_print(", ")
            printer.print(self.vararg.typ)
            printer.plain_print(", ...")
        printer.plain_print("]")

    def __getitem__(self, typ: TypeVarValue | tuple[TypeVarValue, ...]) -> "Generic":
        return self.where(typ)

    def where(self, typ: TypeVarValue | tuple[TypeVarValue, ...]) -> "Generic":
        if isinstance(typ, tuple):
            typs = typ
        else:
            typs = (typ,)

        args, vararg = _split_type_args(typs)
        if self.vararg is None and vararg is None:
            assert len(args) <= len(
                self.vars
            ), "Number of type arguments does not match"
            if all(v.is_subseteq(bound) for v, bound in zip(args, self.vars)):
                return Generic(self.body, *args, *self.vars[len(args) :])
            else:
                raise TypeError("Type arguments do not match")
        elif self.vararg is not None and vararg is None:
            assert len(args) >= len(
                self.vars
            ), "Number of type arguments does not match"
            if all(v.is_subseteq(bound) for v, bound in zip(args, self.vars)) and all(
                v.is_subseteq(self.vararg.typ) for v in args[len(self.vars) :]
            ):
                return Generic(self.body, *args)
        elif self.vararg is not None and vararg is not None:
            if len(args) < len(self.vars):
                if (
                    all(v.is_subseteq(bound) for v, bound in zip(args, self.vars))
                    and all(
                        vararg.typ.is_subseteq(bound)
                        for bound in self.vars[len(args) :]
                    )
                    and vararg.typ.is_subseteq(self.vararg.typ)
                ):
                    return Generic(self.body, *args, vararg)
            else:
                if (
                    all(v.is_subseteq(bound) for v, bound in zip(args, self.vars))
                    and all(v.is_subseteq(vararg.typ) for v in args[len(self.vars) :])
                    and vararg.typ.is_subseteq(self.vararg.typ)
                ):
                    return Generic(self.body, *args, vararg)
        raise TypeError("Type arguments do not match")


HintedData = typing.TypeVar("HintedData")


@typing.final
@dataclass
class Hinted(TypeAttribute, typing.Generic[HintedData]):
    """Type wrapped with a hint.

    `Hinted` is used to represent a type with additional data that can be used as
    a hint for type inference. The additional data is only used for specific type
    inference purposes, or improve certain type inference precision, it does not affect the
    order of types in the lattice.
    """

    name = "Hinted"
    type: TypeAttribute
    data: HintedData

    def __init__(self, type: TypeAttribute, data: HintedData):
        self.data = data
        if isinstance(type, Hinted):
            type = type.type
        self.type = type

    def is_equal(self, other: TypeAttribute) -> bool:
        return (
            isinstance(other, Hinted)
            and self.data == other.data
            and self.type.is_equal(other.type)
        )

    def is_subseteq_fallback(self, other: TypeAttribute) -> bool:
        return self.type.is_subseteq(other)

    def __hash__(self) -> int:
        return hash(self.type)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self, prefix="!")
        printer.plain_print("(")
        printer.print(self.type)
        printer.plain_print(", ", self.data)
        printer.plain_print(")")


def _typeparams_list2tuple(args: tuple[TypeVarValue, ...]) -> tuple[TypeOrVararg, ...]:
    "provides the syntax sugar [A, B, C] type Generic(tuple, A, B, C)"
    return tuple(Generic(tuple, *arg) if isinstance(arg, list) else arg for arg in args)


def _split_type_args(
    args: tuple[TypeVarValue, ...]
) -> tuple[tuple[TypeAttribute, ...], Vararg | None]:
    args = _typeparams_list2tuple(args)
    if args is None or len(args) == 0:
        return (), None

    if isinstance(args[-1], Vararg):
        xs = args[:-1]
        if is_tuple_of(xs, TypeAttribute):
            return xs, args[-1]
        else:
            raise TypeError("Multiple varargs are not allowed")
    elif is_tuple_of(args, TypeAttribute):
        return args, None
    raise TypeError("Vararg must be the last argument")


T = typing.TypeVar("T")


def is_tuple_of(xs: tuple, typ: type[T]) -> typing.TypeGuard[tuple[T, ...]]:
    return all(isinstance(x, typ) for x in xs)


def hint2type(hint) -> TypeAttribute:
    if isinstance(hint, TypeAttribute):
        return hint
    elif hint is None:
        return PyClass(type(None))

    bear_hint = TypeHint(hint)
    if isinstance(bear_hint, TypeVarTypeHint):
        return TypeVar(
            hint.__name__,
            hint2type(hint.__bound__) if hint.__bound__ else None,
        )
    elif isinstance(bear_hint, ClassTypeHint):
        return PyClass(hint)
    elif isinstance(bear_hint, TupleVariableTypeHint):
        if len(bear_hint.args) != 1:
            raise TypeError("Tuple hint must have exactly one argument")
        return Generic(tuple, Vararg(hint2type(bear_hint.args[0])))

    origin: type | None = typing.get_origin(hint)
    if origin is None:  # non-generic
        return PyClass(hint)

    body = PyClass(origin)
    args = typing.get_args(hint)
    params = []
    for arg in args:
        params.append(hint2type(arg))
    return Generic(body, *params)


def unwrap_hinted(hint: TypeAttribute) -> TypeAttribute:
    if isinstance(hint, Hinted):
        return hint.type
    return hint


Any = AnyType()
Bottom = BottomType()
Int = PyClass(int)
Float = PyClass(float)
String = PyClass(str)
Bool = PyClass(bool)
NoneType = PyClass(type(None))
Slice = Generic(slice, TypeVar("T"))
List = Generic(list, TypeVar("T"))
Tuple = Generic(tuple, Vararg(TypeVar("T")))
Dict = Generic(dict, TypeVar("K"), TypeVar("V"))
Set = Generic(set, TypeVar("T"))
FrozenSet = Generic(frozenset, TypeVar("T"))
TypeofFunctionType = Generic[type(lambda: None)]
FunctionType = Generic(type(lambda: None), Tuple, Vararg(Any))
