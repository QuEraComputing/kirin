import io

from rich.console import Console

from kirin.codegen.print import Printer
from kirin.dialects.py import data, types
from kirin.prelude import basic


@basic
def move_gen(start, stop):
    def foo(aod):
        def moo(aod):
            return start, aod

        return moo

    return foo(stop)


@basic
def unstable(x: int):  # type: ignore
    y = x + 1
    if y > 10:
        z = y
    else:
        z = y + 1.2
    return z


class TestBasicPrint:

    def check_print(self, node, *text: str):
        printer = Printer(basic)
        with printer.string_io() as stream:
            printer.emit(node)
            answer = stream.getvalue()
            for txt in text:
                assert self.rich_str(txt) in answer

    def rich_str(self, text: str):
        try:
            file = io.StringIO()
            console = Console()
            console.file = file
            console.print(text, sep="", end="", highlight=False)
            return file.getvalue()
        finally:
            file.close()

    def test_pytypes(self):
        self.check_print(types.Int, "![dark_blue]py[/dark_blue].class.int")
        self.check_print(types.Any, "![dark_blue]py[/dark_blue].Any")
        self.check_print(types.Tuple, "![dark_blue]py[/dark_blue].class.tuple", "~T")
        self.check_print(
            types.PyVararg(types.Int), "*![dark_blue]py[/dark_blue].class.int"
        )
        self.check_print(
            types.PyConst(1),
            "![dark_blue]py.types[/dark_blue].Const(1, ![dark_blue]py[/dark_blue].class.int)",
        )
        self.check_print(
            types.PyUnion(types.Int, types.Float),
            "![dark_blue]py.types[/dark_blue].Union",
            "![dark_blue]py[/dark_blue].class.int",
            "![dark_blue]py[/dark_blue].class.float",
        )

        self.check_print(
            data.PyAttr(1),
            "1[bright_black] : [/bright_black]",
            "[bright_black]![dark_blue]py[/dark_blue].class.int[/bright_black]",
        )

        # TODO: actually test these
        Printer(basic).emit(move_gen)
        Printer(basic).emit(unstable)
