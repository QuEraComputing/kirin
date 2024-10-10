from __future__ import annotations

import typing
from abc import abstractmethod
from dataclasses import dataclass
from typing import Any, Generic, Iterable, NoReturn, TypeVar, Union

from beartype.door import TupleVariableTypeHint  # type: ignore
from beartype.door import ClassTypeHint, TypeHint, TypeVarTypeHint
from typing_extensions import Never

from kirin.ir import (
    AnyType,
    Attribute,
    BottomType,
    TypeAttribute,
    TypeAttributeMeta,
    UnionTypeMeta,
)
from kirin.lattice import Lattice
from kirin.print.printer import Printer

from .dialect import dialect

Type = TypeVar("Type")


@dataclass
class PyType(TypeAttribute["PyType"]):

    @abstractmethod
    def join(self, other: Lattice[PyType]) -> Lattice[PyType]: ...

    @abstractmethod
    def meet(self, other: Lattice[PyType]) -> Lattice[PyType]: ...

    @classmethod
    def top(cls):
        return PyAnyType()

    @classmethod
    def bottom(cls):
        return PyBottomType()

    @abstractmethod
    def __hash__(self) -> int: ...

    @abstractmethod
    def is_subseteq(self, other: Lattice[PyType]) -> bool: ...


@dialect.register
@dataclass
class PyAnyType(AnyType[PyType], PyType):
    name = "Any"

    def is_subseteq(self, other: Lattice[PyType]) -> bool:
        if self is other:
            return True
        elif isinstance(other, PyTypeVar):
            return isinstance(other.bound, AnyType)
        return False

    def __hash__(self) -> int:
        return id(self)

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print("!")
        printer.plain_print("py", style=printer.color.dialect)
        printer.plain_print(".Any")


@dialect.register
@dataclass
class PyBottomType(BottomType, PyType):
    name = "Bottom"

    def __hash__(self) -> int:
        return id(self)

    def is_subseteq(self, other: Lattice) -> bool:
        return True

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print("!")
        printer.plain_print("py", style=printer.color.dialect)
        printer.plain_print(".Bottom")


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


@dialect.register
@dataclass(init=False)
class PyConst(PyType, Generic[Type]):
    """This will not appear in user-space. It is used to
    pass around constant values interprocedurally.
    """

    name = "Const"
    data: Type
    typ: PyType

    def __init__(self, data: Type, typ: PyType | None = None):
        self.data = data
        if isinstance(typ, PyConst):
            raise TypeError("Cannot nest PyConst")

        if typ is not None:
            self.typ = typ
        else:
            self.typ = PyClass(type(data))

    def is_equal(self, other: Lattice[PyType]) -> bool:
        return isinstance(other, PyConst) and self.data == other.data

    def is_subseteq(self, other: Lattice[PyType]) -> bool:
        if other.is_top():
            return True
        elif other.is_bottom():
            return False
        elif isinstance(other, PyConst):
            return self.is_equal(other)
        return self.typ.is_subseteq(other)

    def join(self, other: Lattice[PyType]) -> Lattice[PyType]:
        if self.is_equal(other):
            return self
        else:  # widen constant
            return PyUnion(self.typ, other)

    def meet(self, other: Lattice[PyType]) -> Lattice[PyType]:
        if self.is_subseteq(other):
            return self
        else:
            return PyBottomType()

    def __hash__(self) -> int:
        return hash(self.typ)

    def __repr__(self) -> str:
        return f"Const({self.data})"

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self, prefix="!")
        printer.plain_print("(", self.data, ", ")
        printer.print(self.typ)
        printer.plain_print(")")


@dialect.register
@dataclass
class PyLiteral(PyType, Generic[Type], metaclass=LiteralMeta):
    name = "Literal"
    data: Type

    def is_equal(self, other: Lattice[PyType]) -> bool:
        return self is other  # instance are identical

    def is_subseteq(self, other: Lattice[PyType]) -> bool:
        if other.is_top():
            return True
        elif isinstance(other, PyTypeVar):
            return self.is_subseteq(other.bound)
        elif isinstance(other, PyUnion):
            return any(self.is_subseteq(a) for a in other.args)
        return self.is_equal(other)

    def join(self, other: Lattice[PyType]) -> Lattice[PyType]:
        return PyUnion(self, other)

    def meet(self, other: Lattice[PyType]) -> Lattice[PyType]:
        if self.is_equal(other):
            return self
        else:
            return PyBottomType()

    def __hash__(self) -> int:
        return id(self)

    def __repr__(self) -> str:
        return "Literal(" + repr(self.data) + ")"

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(repr(self.data))


@dialect.register
@dataclass(init=False)
class PyUnion(PyType, metaclass=UnionTypeMeta):
    name = "Union"
    args: frozenset[Lattice[PyType]]

    def __init__(
        self,
        typ_or_set: Lattice[PyType] | Iterable[Lattice[PyType]],
        *typs: Lattice[PyType],
    ) -> None:
        if isinstance(typ_or_set, Lattice):
            params: Iterable[Lattice[PyType]] = (typ_or_set, *typs)
        else:
            params = typ_or_set
            assert not typs, "Cannot pass multiple arguments when passing a set"

        args: frozenset[Lattice[PyType]] = frozenset()
        for typ in params:
            if isinstance(typ, PyUnion):
                args = args.union(typ.args)
            else:
                args = args.union({typ})
        self.args = args

    def is_equal(self, other: Lattice[PyType]) -> bool:
        return isinstance(other, PyUnion) and self.args == other.args

    def is_subseteq(self, other: Lattice[PyType]) -> bool:
        if other.is_top():
            return True
        elif other.is_bottom():
            return False
        return all(a.is_subseteq(other) for a in self.args)

    def join(self, other: Lattice[PyType]) -> Lattice[PyType]:
        if isinstance(other, PyUnion):
            return PyUnion(self.args | other.args)
        return PyUnion(*(self.args | {other}))

    def meet(self, other: Lattice[PyType]) -> Lattice[PyType]:
        if isinstance(other, PyUnion):
            return PyUnion(self.args | other.args)
        return PyUnion(*(self.args & {other}))

    def __hash__(self) -> int:
        return hash((PyUnion, self.args))

    def __repr__(self) -> str:
        return f"PyUnion[{', '.join(map(repr, self.args))}]"

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self, prefix="!")
        printer.print_seq(self.args, delim=", ", prefix="[", suffix="]")


@dialect.register
@dataclass
class PyTypeVar(PyType):
    name = "TypeVar"
    varname: str
    bound: PyType

    def __init__(
        self,
        name: str,
        bound: PyType | None = None,
    ) -> None:
        self.varname = name
        self.bound = bound or PyAnyType()

    def is_equal(self, other: Lattice[PyType]) -> bool:
        return (
            isinstance(other, PyTypeVar)
            and self.varname == other.varname
            and self.bound == other.bound
        )

    def __hash__(self) -> int:
        return hash((PyTypeVar, self.varname, self.bound))

    def __repr__(self) -> str:
        if not self.bound.is_top():
            upper = f" : {self.bound}"
        else:
            upper = ""
        return f"~{self.varname}{upper}"

    def is_subseteq(self, other: Lattice[PyType]) -> bool:
        if other.is_top():
            return True
        elif other.is_bottom():
            return False
        elif isinstance(other, PyTypeVar):
            return self.bound.is_subseteq(other.bound)
        elif isinstance(other, PyUnion):
            return any(self.is_subseteq(a) for a in other.args)
        return self.bound.is_subseteq(other)

    def is_subtype(self, other: Lattice[PyType]) -> bool:
        raise ValueError("TypeVar cannot be used in is_subtype")

    def join(self, other: Lattice[PyType]) -> Lattice[PyType]:
        return PyUnion(self, other)

    def meet(self, other: Lattice[PyType]) -> Lattice[PyType]:
        if self.is_subseteq(other):
            return self
        return PyBottomType()

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print(f"~{self.varname}")
        if not self.bound.is_top():
            printer.plain_print(" : ")
            printer.print(self.bound)


@dialect.register
@dataclass
class PyVararg(Attribute):
    name = "Vararg"
    typ: PyType | PyTypeVar

    def __eq__(self, value: object) -> bool:
        return self is value

    def __hash__(self) -> int:
        return hash((PyVararg, self.typ))

    def __repr__(self) -> str:
        return f"Vararg[{self.typ}]"

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print("*")
        printer.print(self.typ)


PyTypeVarValue = Union[PyType, PyVararg]
PyClassArgs = tuple[PyTypeVarValue, ...] | None


class PyClassMeta(TypeAttributeMeta):

    def __init__(self, *args, **kwargs):
        super(PyClassMeta, self).__init__(*args, **kwargs)
        self._cache = {}

    def __call__(self, typ):
        if typ is Any:
            return PyAnyType()
        elif typ is NoReturn or typ is Never:
            return PyBottomType()
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


@dialect.register
@dataclass(init=False)
class PyClass(PyType, Generic[Type], metaclass=PyClassMeta):
    name = "class"
    typ: type[Type]

    def __init__(self, typ: type[Type]):
        if isinstance(typ, type):
            self.typ = typ
        else:
            raise TypeError(f"Unexpected type {typ}")

    def join(self, other: Lattice[PyType]) -> Lattice[PyType]:
        return PyUnion(self, other)

    def meet(self, other: Lattice[PyType]) -> Lattice[PyType]:
        if self.is_subseteq(other):
            return self
        elif other.is_subseteq(self):
            return other
        return PyBottomType()

    def is_subseteq(self, other: Lattice[PyType]) -> bool:
        if other.is_top():
            return True
        elif other.is_bottom():
            return False
        elif isinstance(other, PyLiteral):
            return False
        elif isinstance(other, PyClass):
            return issubclass(self.typ, other.typ)
        elif isinstance(other, PyUnion):
            return any(self.is_subseteq(a) for a in other.args)
        elif isinstance(other, PyGeneric):
            # NOTE: subclass without generics is just generic with all any parameters
            PyAny = PyAnyType()
            return (
                self.is_subseteq(other.body)
                and all(PyAny.is_subseteq(bound) for bound in other.vars)
                and (other.vararg is None or PyAny.is_subseteq(other.vararg.typ))
            )
        elif isinstance(other, PyTypeVar):
            return self.is_subseteq(other.bound)
        elif isinstance(other, PyConst):
            return self.is_subseteq(other.typ)
        raise TypeError(f"Unexpected type {other}")

    def __hash__(self) -> int:
        return hash((PyClass, self.typ))

    def print_impl(self, printer: Printer) -> None:
        printer.plain_print("!")
        printer.plain_print("py", style=printer.color.dialect)
        printer.plain_print(".", self.name, ".", self.typ.__name__)


@dialect.register
@dataclass(init=False)
class PyGeneric(PyType, Generic[Type]):
    name = "generic"
    body: PyClass[Type]
    vars: tuple[PyType, ...]
    vararg: PyVararg | None = None
    """unknown type variables
    """

    def __init__(self, body: type[Type] | PyClass[Type], *vars: PyType | PyVararg):
        if isinstance(body, PyClass):
            self.body = body
        else:
            self.body = PyClass(body)

        validate_if_last_arg_vararg(vars)
        if vars and isinstance(vars[-1], PyVararg):
            self.vars = vars[:-1]  # type: ignore
            self.vararg = vars[-1]
        else:
            self.vars = vars  # type: ignore

    def is_subseteq(self, other: Lattice[PyType]) -> bool:
        if other.is_top():
            return True
        elif other.is_bottom():
            return False
        elif isinstance(other, PyLiteral):
            return False
        elif isinstance(other, PyGeneric):
            return self.is_subseteq_PyGeneric(other)
        elif isinstance(other, PyClass):
            return self.body.is_subseteq(other)
        elif isinstance(other, PyUnion):
            return any(self.is_subseteq(a) for a in other.args)
        elif isinstance(other, PyTypeVar):
            return self.is_subseteq(other.bound)
        raise TypeError(f"Unexpected type {other}")

    def is_subseteq_PyGeneric(self, other: PyGeneric):
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

    def join(self, other: Lattice[PyType]) -> Lattice[PyType]:
        return PyUnion(self, other)

    def meet(self, other: Lattice[PyType]) -> Lattice[PyType]:
        if self.is_subseteq(other):
            return self
        elif other.is_subseteq(self):
            return other
        return PyBottomType()

    def __hash__(self) -> int:
        return hash((PyGeneric, self.body, self.vars, self.vararg))

    def __repr__(self) -> str:
        from kirin.print import Printer

        return self.print_str(Printer()).splitlines()[0]

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

    def __getitem__(
        self, typ: PyTypeVarValue | tuple[PyTypeVarValue, ...]
    ) -> PyGeneric:
        return self.where(typ)

    def where(self, typ: PyTypeVarValue | tuple[PyTypeVarValue, ...]) -> PyGeneric:
        if isinstance(typ, tuple):
            typs = typ
        else:
            typs = (typ,)

        validate_if_last_arg_vararg(typs)
        if typs and isinstance(typs[-1], PyVararg):
            args: tuple[PyType] = typs[:-1]  # type: ignore
            vararg = typs[-1]
        else:
            args: tuple[PyType] = typs  # type: ignore
            vararg = None

        if self.vararg is None and vararg is None:
            assert len(args) <= len(
                self.vars
            ), "Number of type arguments does not match"
            if all(v.is_subseteq(bound) for v, bound in zip(args, self.vars)):
                return PyGeneric(self.body, *args, *self.vars[len(args) :])
            else:
                raise TypeError("Type arguments do not match")
        elif self.vararg is not None and vararg is None:
            assert len(args) >= len(
                self.vars
            ), "Number of type arguments does not match"
            if all(v.is_subseteq(bound) for v, bound in zip(args, self.vars)) and all(
                v.is_subseteq(self.vararg.typ) for v in args[len(self.vars) :]
            ):
                return PyGeneric(self.body, *args)
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
                    return PyGeneric(self.body, *args, vararg)
            else:
                if (
                    all(v.is_subseteq(bound) for v, bound in zip(args, self.vars))
                    and all(v.is_subseteq(vararg.typ) for v in args[len(self.vars) :])
                    and vararg.typ.is_subseteq(self.vararg.typ)
                ):
                    return PyGeneric(self.body, *args, vararg)
        raise TypeError("Type arguments do not match")


def validate_if_last_arg_vararg(args: PyClassArgs) -> None:
    if args is None or len(args) == 0:
        return

    if isinstance(args[-1], PyVararg):
        for arg in args[:-1]:
            if isinstance(arg, PyVararg):
                raise TypeError("Multiple varargs are not allowed")
    elif any(isinstance(arg, PyVararg) for arg in args):
        raise TypeError("Vararg must be the last argument")


def hint2type(hint):
    if isinstance(hint, PyType):
        return hint
    elif hint is None:
        return PyClass(type(None))

    bear_hint = TypeHint(hint)
    if isinstance(bear_hint, TypeVarTypeHint):
        return PyTypeVar(
            hint.__name__,
            hint2type(hint.__bound__) if hint.__bound__ else None,
        )
    elif isinstance(bear_hint, ClassTypeHint):
        return PyClass(hint)
    elif isinstance(bear_hint, TupleVariableTypeHint):
        if len(bear_hint.args) != 1:
            raise TypeError("Tuple hint must have exactly one argument")
        return PyGeneric(tuple, PyVararg(hint2type(bear_hint.args[0])))

    origin: type | None = typing.get_origin(hint)
    if origin is None:  # non-generic
        return PyClass(hint)

    body = PyClass(origin)  # type: ignore
    args = typing.get_args(hint)
    params = []
    for arg in args:
        params.append(hint2type(arg))
    return PyGeneric(body, *params)


def widen_const(typ: PyType):
    if isinstance(typ, PyConst):
        return typ.typ
    return typ
