from dataclasses import field, dataclass

from attrs import Beer
from stmts import Pour, Puke, Drink, NewBeer, RandomBranch
from dialect import dialect

import lattice as latt
from kirin import ir, interp
from kirin.analysis import Forward
from kirin.dialects import cf, py, func
from kirin.dialects.py import binop
from kirin.analysis.const import Propagate, JointResult


@dataclass
class FeeAnalysis(Forward[latt.Item]):
    keys = ["beer.fee"]
    lattice = latt.Item
    # constprop_results: dict[ir.SSAValue, JointResult] = field(default_factory=dict)
    item_count: int = field(init=False)

    def __post_init__(self) -> None:
        super().__post_init__()

    def initialize(self):
        super().initialize()
        self.item_count = 0
        # self.constprop_results = {}
        return self

    def should_exec_stmt(self, stmt: ir.Statement):
        return isinstance(
            stmt,
            (
                Drink,
                Pour,
                Puke,
            ),
        )

    def run_method(self, method: ir.Method, args: tuple[latt.Item, ...]) -> latt.Item:
        return self.run_callable(method.code, (self.lattice.bottom(),) + args)


@func.dialect.register(key="beer.fee")
class PyFuncMethodTable(interp.MethodTable):

    @interp.impl(func.Call)
    def call(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: func.Call,
    ):
        frame.get(stmt.callee)
        return ()


@binop.dialect.register(key="beer.fee")
class PyBinOpMethodTable(interp.MethodTable):

    @interp.impl(binop.Add)
    def add(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: binop.Add,
    ):
        left: latt.ConstIntItem = frame.get(stmt.lhs)
        right: latt.ConstIntItem = frame.get(stmt.rhs)

        out = latt.ConstIntItem(data=left.value + right.value)
        return (out,)


@dialect.register(key="beer.fee")
class BeerMethodTable(interp.MethodTable):

    menu_price: dict[str, float] = {
        "budlight": 1.0,
        "heineken": 2.0,
        "tsingdao": 3.0,
    }

    @interp.impl(Drink)
    def drink(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: Drink,
    ):
        # Drink depends on the beer type to have different charge:

        beer_runtime: Beer = interp.constprop_results.get(stmt.beverage).const.data
        print("drink")
        interp.item_count += 1
        out = latt.DrinkFee(
            beer_name=beer_runtime.brand, price=self.menu_price[beer_runtime.brand]
        )

        return (out,)

    @interp.impl(Pour)
    def pour(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: Pour,
    ):
        # pour change same rate for all beer types

        amount: int = interp.constprop_results.get(stmt.amount).const.data
        assert isinstance(amount, int)
        interp.item_count += 1
        out = latt.PourFee(count=amount)
        return (out,)

    @interp.impl(Puke)
    def puke(
        self,
        interp: FeeAnalysis,
        frame: interp.Frame[latt.Item],
        stmt: Puke,
    ):
        # puke change same rate for all beer types
        return (latt.PukePenalty(),)
