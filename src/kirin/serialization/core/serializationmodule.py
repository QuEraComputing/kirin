from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.serialization.core.serializationunit import SerializationUnit


class SerializationModule:
    body: "SerializationUnit"
    version: str

    def __init__(
        self,
        body: "SerializationUnit",
        version: str = "",
    ):
        self.body = body
        self.version = version

    def check_version(self, expect_version: str) -> bool:
        return self.version == expect_version
