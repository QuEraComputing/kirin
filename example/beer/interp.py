from random import randint

from attrs import Food, Serving
from stmts import Cook, Nap, Eat, NewFood, RandomBranch
from dialect import dialect

from kirin.interp import Frame, Successor, Interpreter, MethodTable, impl


@dialect.register
class FoodMethods(MethodTable):

    @impl(NewFood)
    def new_food(self, interp: Interpreter, frame: Frame, stmt: NewFood):
        return (Food(stmt.type),)

    @impl(Eat)
    def eat(self, interp: Interpreter, frame: Frame, stmt: Eat):
        serving: Serving = frame.get(stmt.target)
        print(f"Eating {serving.amount} servings of {serving.kind.type}")
        return ()

    @impl(Cook)
    def cook(self, interp: Interpreter, frame: Frame, stmt: Cook):
        food: Food = frame.get(stmt.target)
        amount: int = frame.get(stmt.amount)
        print(f"Cooking {food.type} {amount}")

        return (Serving(food, amount),)

    @impl(Nap)
    def nap(self, interp: Interpreter, frame: Frame, stmt: Nap):
        print("Napping!!!")
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
