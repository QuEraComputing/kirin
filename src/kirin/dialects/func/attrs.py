from typing import Any, Dict, Generic, TypeVar
from dataclasses import dataclass

from kirin import types
from kirin.ir import Method, Attribute
from kirin.print.printer import Printer
from kirin.serialization.base.serializermixin import SerializerMixin

from ._dialect import dialect

TypeofMethodType = types.PyClass[Method]
MethodType = types.Generic(
    Method, types.TypeVar("Params", types.Tuple), types.TypeVar("Ret")
)
TypeLatticeElem = TypeVar("TypeLatticeElem", bound="types.TypeAttribute")


@dialect.register
@dataclass
class Signature(Generic[TypeLatticeElem], Attribute, SerializerMixin):
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

    def serialize(self) -> dict[str, Any]:
        # return the plain inner payload ONLY; global Serializer will encode nested
        return {"inputs": [a for a in self.inputs], "output": self.output}

    @classmethod
    def deserialize(cls, data: Dict[str, Any]) -> "Signature":
        # accept the plain inner payload produced by serialize()
        inputs = tuple(data.get("inputs", ()))
        output = data.get("output")
        return cls(inputs=inputs, output=output)
