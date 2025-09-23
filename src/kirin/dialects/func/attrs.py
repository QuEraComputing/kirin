from typing import TYPE_CHECKING, Generic, TypeVar
from dataclasses import dataclass

from kirin import types
from kirin.ir import Method, Attribute
from kirin.print.printer import Printer
from kirin.serialization.base.serializationunit import SerializationUnit

if TYPE_CHECKING:
    from kirin.serialization.base.serializer import Serializer
    from kirin.serialization.base.deserializer import Deserializer

from ._dialect import dialect

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

    def __eq__(self, value: object) -> bool:
        if not isinstance(value, Signature):
            return False
        return self.inputs == value.inputs and self.output == value.output

    def serialize(self, serializer: "Serializer") -> "SerializationUnit":
        return SerializationUnit(
            kind="attribute",
            module_name=self.__class__.__module__,
            class_name=self.__class__.__name__,
            data={
                "inputs": serializer.serialize(self.inputs),
                "output": serializer.serialize(self.output),
            },
        )

    @classmethod
    def deserialize(
        cls, serUnit: "SerializationUnit", deserializer: "Deserializer"
    ) -> "Signature":
        inputs = deserializer.deserialize(serUnit.data["inputs"])
        output = deserializer.deserialize(serUnit.data["output"])
        return cls(inputs=inputs, output=output)
