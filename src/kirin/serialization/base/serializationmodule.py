from kirin.serialization.base.context import MethodSymbolMeta
from kirin.serialization.base.serializationunit import SerializationUnit


class SerializationModule:
    symbol_table: dict[str, MethodSymbolMeta]
    body: SerializationUnit

    def __init__(
        self, symbol_table: dict[str, MethodSymbolMeta], body: SerializationUnit
    ):
        self.symbol_table = symbol_table
        self.body = body
