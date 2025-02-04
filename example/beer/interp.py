from random import randint

from attrs import Beer, Pints
from stmts import Pour, Puke, Drink, NewBeer, RandomBranch
from dialect import dialect

from kirin.interp import Frame, Successor, Interpreter, MethodTable, impl


@dialect.register
class BeerMethods(MethodTable):

    @impl(NewBeer)
    def new_beer(self, interp: Interpreter, frame: Frame, stmt: NewBeer):
        return (Beer(stmt.brand),)

    @impl(Drink)
    def drink(self, interp: Interpreter, frame: Frame, stmt: Drink):
        pints: Pints = frame.get(stmt.pints)
        print(f"Drinking {pints.amount} pints of {pints.kind.brand}")
        return ()

    @impl(Pour)
    def pour(self, interp: Interpreter, frame: Frame, stmt: Pour):
        beer: Beer = frame.get(stmt.beverage)
        amount: int = frame.get(stmt.amount)
        print(f"Pouring {beer.brand} {amount}")

        return (Pints(beer, amount),)

    @impl(Puke)
    def puke(self, interp: Interpreter, frame: Frame, stmt: Puke):
        print("Puking!!!")
        return ()

    @impl(RandomBranch)
    def random_branch(self, interp: Interpreter, frame: Frame, stmt: RandomBranch):
        frame = interp.state.current_frame()
        if randint(0, 1):
            return Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
        else:
            return Successor(
                stmt.else_successor, *frame.get_values(stmt.then_arguments)
            )
