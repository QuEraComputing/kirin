import io

from rich.console import Console

from kirin import ir, types
from kirin.print import Printer
from kirin.prelude import basic
from kirin.dialects import py, func


@basic
def move_gen(start, stop):
    def foo(aod):
        def moo(aod):
            return start, aod

        py.Map(moo, aod)  # type: ignore
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


@basic
def empty():
    pass


class TestBasicPrint:

    def dummy_check(self, node):
        printer = Printer()
        printer.print(node)
        printer.plain_print("\n")

    def check_print(self, node, *text: str):
        printer = Printer()
        with printer.string_io() as stream:
            printer.print(node)
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
        self.check_print(types.Int, "![dark_blue]py[/dark_blue].int")
        self.check_print(types.Any, "!Any")
        self.check_print(types.Tuple, "![dark_blue]py[/dark_blue].tuple", "~T")
        self.check_print(types.Vararg(types.Int), "*![dark_blue]py[/dark_blue].int")
        self.check_print(
            types.Int,
            "![dark_blue]py[/dark_blue].int",
        )
        self.check_print(
            types.Union(types.Int, types.Float),
            "!Union",
            "![dark_blue]py[/dark_blue].int",
            "![dark_blue]py[/dark_blue].float",
        )

        self.check_print(
            ir.PyAttr(1),
            "1[bright_black] : [/bright_black]",
            "[bright_black]![dark_blue]py[/dark_blue].int[/bright_black]",
        )

        # TODO: actually test these
        self.dummy_check(move_gen)
        self.dummy_check(unstable)
        self.dummy_check(empty)
        self.dummy_check(empty.code)
        assert isinstance(empty.code, func.Function)
        assert isinstance(empty.callable_region, ir.Region)
        region = empty.callable_region
        self.dummy_check(region.blocks[0])
        empty.callable_region.blocks[0].detach()
        self.dummy_check(empty.code)
