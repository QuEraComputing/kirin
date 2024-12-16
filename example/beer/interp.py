from random import randint

from attrs import Beer
from stmts import Pour, Puke, Drink, NewBeer, RandomBranch
from dialect import dialect

from kirin.interp import Frame, Successor, Interpreter, MethodTable, impl


@dialect.register
class BeerInterpreter(MethodTable):

    @impl(NewBeer)
    def new_beer(self, interp: Interpreter, frame: Frame, stmt: NewBeer):
        return (Beer(frame.get(stmt.brand)),)

    @impl(Drink)
    def drink(self, interp: Interpreter, frame: Frame, stmt: Drink):
        print(f"Drinking {frame.get(stmt.beverage).brand}")
        return ()

    @impl(Pour)
    def pour(self, interp: Interpreter, frame: Frame, stmt: Pour):
        print(f"Pouring {frame.get(stmt.beverage).brand} {frame.get(stmt.amount)}")
        return ()

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
