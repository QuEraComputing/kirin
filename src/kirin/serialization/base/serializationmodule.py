from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.serialization.base.context import MethodSymbolMeta
    from kirin.serialization.serializationunit import SerializationUnit


class SerializationModule:
    symbol_table: dict[str, "MethodSymbolMeta"]
    body: "SerializationUnit"

    def __init__(
        self, symbol_table: dict[str, "MethodSymbolMeta"], body: "SerializationUnit"
    ):
        self.symbol_table = symbol_table
        self.body = body
