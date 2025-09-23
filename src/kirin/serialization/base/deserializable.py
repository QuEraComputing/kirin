from abc import abstractmethod
from typing import TYPE_CHECKING, Self, Protocol, runtime_checkable

if TYPE_CHECKING:
    from kirin.serialization.base.deserializer import Deserializer
    from kirin.serialization.base.serializationunit import SerializationUnit


@runtime_checkable
class Deserializable(Protocol):

    @classmethod
    @abstractmethod
    def deserialize(
        cls: type[Self], serUnit: "SerializationUnit", deserializer: "Deserializer"
    ) -> Self:
        raise NotImplementedError
