from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.ir.nodes.base import IRNode


class InterpreterExit(Exception):
    pass


class DialectDefError(Exception):
    pass


class DialectSyntaxError(Exception):
    pass


class CompilerError(Exception):
    pass


class ForwardedError(Exception):
    def __init__(
        self, node: "IRNode", *messages: str, err: Exception | None = None
    ) -> None:
        super().__init__(*messages)
        self.node = node
        self.err = err


class VerificationError(ForwardedError):
    pass


class TypeCheckError(ForwardedError):
    pass
