from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Any, Dict, Type, TypeVar

if TYPE_CHECKING:
    from kirin.serialization.base.serializer import Serializer
    from kirin.serialization.base.deserializer import Deserializer

T = TypeVar("T", bound="SerializerMixin")


class SerializerMixin(ABC):
    @abstractmethod
    def serialize(self, serializer: "Serializer") -> Dict[str, Any]: ...

    @classmethod
    @abstractmethod
    def deserialize(
        cls: Type[T], data: Dict[str, Any], deserializer: "Deserializer"
    ) -> T: ...
