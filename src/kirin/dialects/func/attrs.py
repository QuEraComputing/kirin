from typing import TYPE_CHECKING, Generic, TypeVar
from dataclasses import dataclass

from kirin import types
from kirin.ir import Attribute
from kirin.print.printer import Printer
from kirin.serialization.core.serializationunit import SerializationUnit

if TYPE_CHECKING:
    from kirin.serialization.base.serializer import Serializer
    from kirin.serialization.base.deserializer import Deserializer

from ._dialect import dialect

# TypeofMethodType = types.PyClass[Method]
# MethodType = types.Generic(
#     Method, types.TypeVar("Params", types.Tuple), types.TypeVar("Ret")
# )
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

    def __eq__(self, value: object) -> bool:
        if not isinstance(value, Signature):
            return False
        return self.inputs == value.inputs and self.output == value.output

    def serialize(self, serializer: "Serializer") -> "SerializationUnit":
        return SerializationUnit(
            kind="signature",
            module_name=self.__module__,
            class_name=self.__class__.__name__,
            data={
                "inputs": serializer.serialize_tuple(self.inputs),
                "output": serializer.serialize(self.output),
            },
        )

    def is_structurally_equal(
        self,
        other: Attribute,
        context: dict | None = None,
    ) -> bool:
        return self == other

    @classmethod
    def deserialize(
        cls, serUnit: "SerializationUnit", deserializer: "Deserializer"
    ) -> "Signature":
        inputs = deserializer.deserialize(serUnit.data["inputs"])
        output = deserializer.deserialize(serUnit.data["output"])
        return Signature(inputs=inputs, output=output)
