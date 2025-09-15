from abc import ABC, abstractmethod
from typing import Any, Dict, Type, TypeVar

T = TypeVar("T", bound="SerializerMixin")


class SerializerMixin(ABC):
    @abstractmethod
    def serialize(self) -> Dict[str, Any]: ...

    @classmethod
    @abstractmethod
    def deserialize(cls: Type[T], data: Dict[str, Any]) -> T: ...

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
