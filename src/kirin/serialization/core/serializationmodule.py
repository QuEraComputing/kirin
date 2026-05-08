from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.serialization.base.context import MethodSymbolMeta
    from kirin.serialization.core.serializationunit import SerializationUnit


class SerializationModule:
    symbol_table: dict[str, "MethodSymbolMeta"]
    body: "SerializationUnit"
    version: str

    def __init__(
        self,
        symbol_table: dict[str, "MethodSymbolMeta"],
        body: "SerializationUnit",
        version: str = "",
    ):
        self.symbol_table = symbol_table
        self.body = body
        self.version = version

    def check_version(self, expect_version: str) -> bool:
        return self.version == expect_version
