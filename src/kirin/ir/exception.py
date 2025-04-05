from typing import TYPE_CHECKING

from kirin.exception import NoPythonStackTrace

if TYPE_CHECKING:
    from kirin.ir.nodes.base import IRNode


class HintedError(NoPythonStackTrace):
    def __init__(self, *messages: str, help: str | None = None) -> None:
        super().__init__(*messages)
        self.help = help


class ValidationError(HintedError):
    def __init__(self, node: "IRNode", *messages: str, help: str | None = None) -> None:
        super().__init__(*messages, help=help)
        self.node = node


class TypeCheckError(ValidationError):
    pass


class CompilerError(Exception):
    pass
