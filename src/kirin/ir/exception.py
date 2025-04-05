from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.ir.nodes.base import IRNode


class ValidationError(Exception):
    def __init__(self, node: "IRNode", *messages: str) -> None:
        super().__init__(*messages)
        self.node = node


class TypeCheckError(ValidationError):
    pass


class CompilerError(Exception):
    pass
