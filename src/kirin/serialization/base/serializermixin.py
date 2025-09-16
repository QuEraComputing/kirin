from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Any, Dict, Type, TypeVar

if TYPE_CHECKING:
    from kirin.serialization.base.serializer import Serializer

T = TypeVar("T", bound="SerializerMixin")


class SerializerMixin(ABC):
    @abstractmethod
    def serialize(self, serializer: "Serializer") -> Dict[str, Any]: ...

    @classmethod
    @abstractmethod
    def deserialize(
        cls: Type[T], data: Dict[str, Any], serializer: "Serializer"
    ) -> T: ...

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)
        try:
            from kirin.serialization.base.registry import register_type
        except Exception:
            return
        try:
            register_type(cls)
        except Exception:
            return
