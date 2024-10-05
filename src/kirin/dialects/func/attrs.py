from dataclasses import dataclass
from typing import Generic, TypeVar

from kirin.dialects.py import types
from kirin.ir import Attribute, Method, TypeAttribute
from kirin.print.printer import Printer

TypeofMethodType = types.PyClass[Method]
MethodType = types.PyGeneric(
    Method, types.PyTypeVar("Params", types.Tuple), types.PyTypeVar("Ret")
)
TypeLatticeElem = TypeVar("TypeLatticeElem", bound="TypeAttribute")


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
        printer.show_function_types(self.inputs, [self.output])
