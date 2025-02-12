from typing import Generic, TypeVar
from dataclasses import dataclass

from kirin import types
from kirin.ir import Method, Attribute
from kirin.print.printer import Printer
from kirin.dialects.func.dialect import dialect

TypeofMethodType = types.PyClass[Method]
MethodType = types.Generic(
    Method, types.TypeVar("Params", types.Tuple), types.TypeVar("Ret")
)
TypeLatticeElem = TypeVar("TypeLatticeElem", bound="types.TypeAttribute")


@dialect.register
@dataclass
class Signature(Generic[TypeLatticeElem], Attribute):
    """function body signature.

    This is not a type attribute because it just stores
    the signature of a function at its definition site.
    We don't perform type inference on this directly.

    The type of a function is the type of `inputs[0]`, which
    typically is a `MethodType`.
    """

    name = "Signature"
    inputs: tuple[TypeLatticeElem, ...]
    output: TypeLatticeElem  # multi-output must be tuple

    def __hash__(self) -> int:
        return hash((self.inputs, self.output))

    def print_impl(self, printer: Printer) -> None:
        printer.print_seq(self.inputs, delim=", ", prefix="(", suffix=")")
        printer.plain_print(" -> ")
        printer.print(self.output)
