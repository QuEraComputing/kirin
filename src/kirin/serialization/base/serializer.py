from abc import ABC, abstractmethod
from typing import Any
from dataclasses import dataclass

from kirin import ir


@dataclass(frozen=True, eq=False)
class Serializer(ABC):
    # Abstract base class for all serializers.
    # Interface from Kirin IR to a serialization format (JSON, binary, etc.)

    @abstractmethod
    def encode(self, mthd: ir.Method) -> Any: ...

    @abstractmethod
    def decode(self, data: Any) -> ir.Method: ...

    @abstractmethod
    def serialize(self, obj: Any) -> dict[str, Any]: ...

    @abstractmethod
    def deserialize(sxwxwelf, data: dict[str, Any]) -> Any: ...

    @abstractmethod
    def serialize_statement(self, stmt: ir.Statement) -> Any: ...

    @abstractmethod
    def deserialize_statement(self, data: Any) -> ir.Statement: ...

    @abstractmethod
    def serialize_block(self, block: ir.Block) -> Any: ...

    @abstractmethod
    def deserialize_block(self, data: Any) -> ir.Block: ...

    @abstractmethod
    def serialize_region(self, region: ir.Region) -> Any: ...

    @abstractmethod
    def deserialize_region(self, data: Any) -> ir.Region: ...

    @abstractmethod
    def serialize_block_argument(self, arg: ir.BlockArgument) -> Any: ...

    @abstractmethod
    def deserialize_block_argument(self, data: Any) -> ir.BlockArgument: ...

    @abstractmethod
    def serialize_ssa_value(self, value: ir.SSAValue) -> Any: ...

    @abstractmethod
    def deserialize_ssa_value(self, data: Any) -> ir.SSAValue: ...

    @abstractmethod
    def serialize_attribute(self, attr: ir.Attribute) -> Any: ...

    @abstractmethod
    def deserialize_attribute(self, data: Any) -> ir.Attribute: ...

    @abstractmethod
    def serialize_result(self, result: ir.ResultValue) -> Any: ...

    @abstractmethod
    def deserialize_result(self, owner: ir.Statement, data: Any) -> ir.ResultValue: ...
