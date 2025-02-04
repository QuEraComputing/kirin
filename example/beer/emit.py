from io import StringIO
from dataclasses import field, dataclass

import stmts
from group import beer
from dialect import dialect

from kirin import ir, interp
from lattice import Item, ItemPints, AtLeastXItem, ConstIntItem
from kirin.emit import EmitStr, EmitStrFrame
from kirin.dialects import func
from kirin.emit.exceptions import EmitError


def default_menu_price():
    return {
        "budlight": 3.0,
        "heineken": 4.0,
        "tsingdao": 2.0,
    }


@dataclass
class EmitReceptMain(EmitStr):
    keys = ["emit.recept"]
    dialects: ir.DialectGroup = field(default=beer)
    file: StringIO = field(default_factory=StringIO)
    menu_price: dict[str, float] = field(default_factory=default_menu_price)
    recept_analysis_result: dict[ir.SSAValue, Item] = field(default_factory=dict)

    def initialize(self):
        super().initialize()
        self.file.truncate(0)
        self.file.seek(0)
        return self

    def eval_stmt_fallback(
        self, frame: EmitStrFrame, stmt: ir.Statement
    ) -> tuple[str, ...]:
        return (stmt.name,)

    def emit_block(self, frame: EmitStrFrame, block: ir.Block) -> str | None:
        for stmt in block.stmts:
            result = self.eval_stmt(frame, stmt)
            if isinstance(result, tuple):
                frame.set_values(stmt.results, result)
        return None

    def get_output(self) -> str:
        self.file.seek(0)
        return "\n".join(
            [
                "item    \tamount \t  price",
                "-----------------------------------",
                self.file.read(),
            ]
        )


@func.dialect.register(key="emit.recept")
class FuncEmit(interp.MethodTable):

    @interp.impl(func.Function)
    def emit_func(self, emit: EmitReceptMain, frame: EmitStrFrame, stmt: func.Function):
        _ = emit.run_ssacfg_region(frame, stmt.body)
        return ()


@dialect.register(key="emit.recept")
class BeerEmit(interp.MethodTable):

    @interp.impl(stmts.Pour)
    def emit_pour(self, emit: EmitReceptMain, frame: EmitStrFrame, stmt: stmts.Pour):
        pints_item: ItemPints = emit.recept_analysis_result[stmt.result]

        amount_str = ""
        price_str = ""
        if isinstance(pints_item.count, AtLeastXItem):
            amount_str = f">={pints_item.count.data}"
            price_str = (
                f"  >=${emit.menu_price[pints_item.brand] * pints_item.count.data}"
            )
        elif isinstance(pints_item.count, ConstIntItem):
            amount_str = f"  {pints_item.count.data}"
            price_str = (
                f"  ${emit.menu_price[pints_item.brand] * pints_item.count.data}"
            )
        else:
            raise EmitError("invalid analysis result.")

        emit.writeln(frame, f"{pints_item.brand}\t{amount_str}\t{price_str}")

        return ()
