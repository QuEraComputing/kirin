from enum import IntEnum, unique
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.serialization.core.serializationunit import SerializationUnit


@unique
class CodecVersion(IntEnum):
    """Kirin-owned serialization codec versions."""

    LEGACY_UNVERSIONED = 1
    V2 = 2


CURRENT_CODEC_VERSION = CodecVersion.V2
SUPPORTED_CODEC_VERSIONS: frozenset[CodecVersion] = frozenset({CURRENT_CODEC_VERSION})


class SerializationModule:
    body: "SerializationUnit"
    codec_version: CodecVersion
    version: str

    def __init__(
        self,
        body: "SerializationUnit",
        version: str = "",
    ):
        self.body = body
        self.codec_version = CURRENT_CODEC_VERSION
        self.version = version

    def check_version(self, expect_version: str) -> bool:
        return self.version == expect_version
