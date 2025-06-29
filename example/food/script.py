# type: ignore
from group import food
from stmts import Eat, Nap, Cook, NewFood
from recept import FeeAnalysis

from emit import EmitReceptMain
from interp import FoodMethods as FoodMethods
from lattice import AtLeastXItem
from rewrite import NewFoodAndNap
from kirin.rewrite import Walk


@food
def main(x: int):
    food = NewFood(type="burger")  # (1)!
    serving = Cook(food, x)  # (2)!
    Eat(serving)  # (3)!
    Nap()  # (4)!

    return x + 1  # (5)!


main.print()


@food
def main2(x: int):
    def some_closure(food, amount):
        Cook(food, amount + 1)
        Nap()

    fish = NewFood(type="fish")
    chicken = NewFood(type="chicken")
    potatoes = NewFood(type="potatoes")

    fish_serving = Cook(fish, 12 + x)
    chicken_serving = Cook(chicken, 10 + x)
    potatoes_serving = Cook(potatoes, 8)
    Eat(fish_serving)
    Nap()

    some_closure(fish, 1 + 1)
    if x > 1:
        Eat(chicken_serving)
    else:
        Eat(potatoes_serving)
    return x + 1


main2.code.print()
main2(1)  # execute the function
# for i in range(10):
#     print("iteration", i)
#     main(i)  # now eat a random food!


# 2. simple rewrite:
@food
def main3():

    sandwich = NewFood(type="sandwich")
    chips = NewFood(type="chips")

    sandwich_serving = Cook(sandwich, 2)
    chips_serving = Cook(chips, 10)

    Eat(sandwich_serving)
    Eat(chips_serving)


main3.print()
Walk(NewFoodAndNap()).rewrite(main3.code)
main3.print()


# 3. simple analysis example:
@food
def analysis_demo(x: int):

    burger = NewFood(type="burger")
    salad = NewFood(type="salad")

    burger_serving = Cook(burger, 12 + x)
    salad_serving = Cook(salad, 10 + x)

    Eat(burger_serving)
    Eat(salad_serving)
    Nap()

    Eat(burger_serving)
    Nap()

    Eat(burger_serving)
    Nap()

    return x


fee_analysis = FeeAnalysis(analysis_demo.dialects)
results, expect = fee_analysis.run_analysis(
    analysis_demo, args=(AtLeastXItem(data=10),)
)
print(results.entries)
print(fee_analysis.nap_count)
analysis_demo.print(analysis=results.entries)


emitter = EmitReceptMain()
emitter.recept_analysis_result = results.entries

emitter.run(analysis_demo, ("",))
print(emitter.get_output())
