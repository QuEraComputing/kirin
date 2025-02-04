from dataclasses import field, dataclass

from attrs import Beer
from stmts import Pour, Puke, NewBeer
from dialect import dialect

import lattice as latt
from kirin import ir, interp
from kirin.interp import exceptions
from kirin.analysis import Forward
from kirin.dialects import py
from kirin.dialects.py import binop
from kirin.analysis.forward import ForwardFrame


@dataclass
class FeeAnalysis(Forward[latt.Item]):
    keys = ["beer.fee"]
    lattice = latt.Item
    puke_count: int = field(init=False)

    def initialize(self):
        """Initialize the analysis pass.

        The method is called before the analysis pass starts.

        Note:
            1. Here one is *required* to call the super().initialize() to initialize the analysis pass,
            which clear all the previous analysis results and symbol tables.
            2. Any additional initialization that belongs to the analysis should also be done here.
            For example, in this case, we initialize the puke_count to 0.

        """
        super().initialize()
        self.puke_count = 0
        return self

    def eval_stmt_fallback(
        self, frame: ForwardFrame[latt.Item, None], stmt: ir.Statement
    ) -> tuple[latt.Item, ...] | interp.SpecialValue[latt.Item]:
        return ()

    def run_method(self, method: ir.Method, args: tuple[latt.Item, ...]) -> latt.Item:
        return self.run_callable(method.code, (self.lattice.bottom(),) + args)


@py.constant.dialect.register(key="beer.fee")
class PyConstMethodTable(interp.MethodTable):

    @interp.impl(py.constant.Constant)
    def const(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: py.constant.Constant,
    ):
        if isinstance(stmt.value, int):
            return (latt.ConstIntItem(data=stmt.value),)
        elif isinstance(stmt.value, Beer):
            return (latt.ItemBeer(brand=stmt.value.brand),)

        else:
            raise exceptions.InterpreterError(
                f"illegal constant type {type(stmt.value)}"
            )


@binop.dialect.register(key="beer.fee")
class PyBinOpMethodTable(interp.MethodTable):

    @interp.impl(binop.Add)
    def add(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: binop.Add,
    ):
        left = frame.get(stmt.lhs)
        right = frame.get(stmt.rhs)

        if isinstance(left, latt.AtLeastXItem) or isinstance(right, latt.AtLeastXItem):
            out = latt.AtLeastXItem(data=left.data + right.data)
        else:
            out = latt.ConstIntItem(data=left.data + right.data)

        return (out,)


@dialect.register(key="beer.fee")
class BeerMethodTable(interp.MethodTable):

    @interp.impl(NewBeer)
    def new_beer(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: NewBeer,
    ):
        return (latt.ItemBeer(brand=stmt.brand),)

    @interp.impl(Pour)
    def pour(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: Pour,
    ):
        # Drink depends on the beer type to have different charge:

        beer: latt.ItemBeer = frame.get(stmt.beverage)
        pint_count: latt.AtLeastXItem | latt.ConstIntItem = frame.get(stmt.amount)

        out = latt.ItemPints(count=pint_count, brand=beer.brand)

        return (out,)

    @interp.impl(Puke)
    def puke(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: Puke,
    ):
        interp.puke_count += 1
        return ()
