from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kirin.lowering import LoweringState
    from kirin.ir.nodes.base import IRNode


class InterpreterExit(Exception):
    pass


class InterpreterError(Exception):
    """An error occurred during interpretation.

    This is the base class for all errors that occur during interpretation. This
    error can be catched by interpreter and converted to an `interp.Err` value that
    records the stack trace of the error.
    """

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
        source = lowering.source
        if lowering.lines:
            lines = lowering.lines
            begin = max(0, source.lineno - lowering.max_lines)
            end = min(
                len(lines),
                source.end_lineno or source.lineno + lowering.max_lines,
            )
            lines = (
                lines[begin : source.lineno]
                + [("-" * source.col_offset) + "^"]
                + lines[lowering.lineno_offset : end]
            )
            trace = "\n".join(lines)
            msg = f"{msg}: \n\n{trace}"
        else:
            msg = f"{msg}: {source.lineno}:{source.col_offset}"

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
