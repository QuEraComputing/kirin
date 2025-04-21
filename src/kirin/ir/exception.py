from __future__ import annotations

import sys
import inspect
import textwrap
from typing import TYPE_CHECKING

from rich.console import Console

from kirin.exception import StaticCheckError
from kirin.print.printer import Printer

if TYPE_CHECKING:
    from kirin.ir import IRNode, Method


class ValidationError(StaticCheckError):
    def __init__(self, node: "IRNode", *messages: str, help: str | None = None) -> None:
        super().__init__(*messages, help=help)
        self.node = node
        self.source = node.source
        self.method: Method | None = None

    def attach(self, method: Method):
        if self.method:
            return
        self.method = method

        from kirin.ir.nodes.stmt import Statement

        console = Console(force_terminal=True, force_jupyter=False, file=sys.stderr)
        printer = Printer(console=console)
        # NOTE: populate the printer with the method body
        with printer.string_io():
            printer.print(method.code)
        with printer.string_io() as io:
            printer.print(self.node)
            node_str = io.getvalue()

        node_str = "\n".join(
            map(lambda each_line: " " * 4 + each_line, node_str.splitlines())
        )
        if isinstance(self.node, Statement):
            dialect = self.node.dialect.name if self.node.dialect else "<no dialect>"
            self.args += (
                "when verifying the following statement",
                f" `{dialect}.{type(self.node).__name__}` at\n",
                f"{node_str}\n",
            )
        else:
            self.args += (
                f"when verifying the following statement `{type(self.node).__name__}` at\n",
                f"{node_str}\n",
            )

        if self.source:
            self.source.lineno_begin = method.lineno_begin

        if self.node.source and method.py_func:  # print hint if we have a source
            source = textwrap.dedent(inspect.getsource(method.py_func))
            self.lines = source.splitlines()
            if self.source and self.source.file is None:
                self.source.file = method.file


class TypeCheckError(ValidationError):
    pass


class CompilerError(Exception):
    pass
