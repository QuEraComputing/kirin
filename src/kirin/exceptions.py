from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.ir.nodes.base import IRNode
    from kirin.lowering import LoweringState


class InterpreterExit(Exception):
    pass


class InterpreterError(Exception):
    pass


class InterpreterStepError(InterpreterError):
    pass


class UnreachableError(InterpreterError):
    pass


class FuelExhaustedError(InterpreterError):
    pass


class CodeGenError(InterpreterError):
    pass


class DialectDefError(Exception):
    pass


class DialectSyntaxError(Exception):
    pass


class DialectInterpretationError(InterpreterError):
    pass


class DialectLoweringError(Exception):

    def append_hint(self, lowering: LoweringState):
        msg = self.args[0]
        if lowering.lines:
            lines = lowering.lines
            begin = max(0, lowering.line_range[0] - lowering.max_lines)
            end = min(len(lines), lowering.line_range[1] + lowering.max_lines)
            lines = (
                lines[begin : lowering.line_range[0]]
                + [("-" * lowering.col_range[0]) + "^"]
                + lines[lowering.line_range[1] : end]
            )
            trace = "\n".join(lines)
            msg = f"{msg}: \n\n{trace}"
        else:
            msg = f"{msg}: {lowering.line_range[0]}:{lowering.col_range[0]}"

        self.args = (msg,)
        return self


class CompilerError(Exception):
    pass


class VerificationError(Exception):
    def __init__(self, node: "IRNode", *messages: str) -> None:
        super().__init__(*messages)
        self.node = node


class DuplicatedDefinitionError(Exception):
    pass
