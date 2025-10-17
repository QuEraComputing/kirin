from abc import abstractmethod
from typing import TYPE_CHECKING, Protocol, runtime_checkable

if TYPE_CHECKING:
    from kirin.serialization.base.serializer import Serializer
    from kirin.serialization.core.serializationunit import SerializationUnit


@runtime_checkable
class Serializable(Protocol):
    @abstractmethod
    def serialize(self, serializer: "Serializer") -> "SerializationUnit":
        raise NotImplementedError
