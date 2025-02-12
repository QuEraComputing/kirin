# type: ignore
from group import beer
from stmts import Pour, Puke, Drink, NewBeer
from recept import FeeAnalysis

from emit import EmitReceptMain
from interp import BeerMethods as BeerMethods
from lattice import AtLeastXItem
from rewrite import NewBeerAndPukeOnDrink
from kirin.rewrite import Walk


@beer
def main(x: int):
    beer = NewBeer(brand="budlight")  # (1)!
    pints = Pour(beer, x)  # (2)!
    Drink(pints)  # (3)!
    Puke()  # (4)!

    return x + 1  # (5)!


main.print()


@beer
def main2(x: int):
    def some_closure(beer, amount):
        Pour(beer, amount + 1)
        Puke()

    bud = NewBeer(brand="budlight")
    heineken = NewBeer(brand="heineken")
    tsingdao = NewBeer(brand="tsingdao")

    bud_pints = Pour(bud, 12 + x)
    heineken_pints = Pour(heineken, 10 + x)
    tsingdao_pints = Pour(tsingdao, 8)
    Drink(bud_pints)
    Puke()

    some_closure(bud, 1 + 1)
    if x > 1:
        Drink(heineken_pints)
    else:
        Drink(tsingdao_pints)
    return x + 1


main2.code.print()
main2(1)  # execute the function
# for i in range(10):
#     print("iteration", i)
#     main(i)  # now drink a random beer!


# 2. simple rewrite:
@beer
def main3():

    bud = NewBeer(brand="budlight")
    heineken = NewBeer(brand="heineken")

    bud_pints = Pour(bud, 2)
    heineken_pints = Pour(heineken, 10)

    Drink(bud_pints)
    Drink(heineken_pints)


main3.print()
Walk(NewBeerAndPukeOnDrink()).rewrite(main3.code)
main3.print()


# 3. simple analysis example:
@beer
def analysis_demo(x: int):

    bud = NewBeer(brand="budlight")
    heineken = NewBeer(brand="heineken")

    bud_pints = Pour(bud, 12 + x)
    heineken_pints = Pour(heineken, 10 + x)

    Drink(bud_pints)
    Drink(heineken_pints)
    Puke()

    Drink(bud_pints)
    Puke()

    Drink(bud_pints)
    Puke()

    return x


fee_analysis = FeeAnalysis(analysis_demo.dialects)
results, expect = fee_analysis.run_analysis(
    analysis_demo, args=(AtLeastXItem(data=10),)
)
print(results.entries)
print(fee_analysis.puke_count)
analysis_demo.print(analysis=results.entries)


emitter = EmitReceptMain()
emitter.recept_analysis_result = results.entries

emitter.run(analysis_demo, ("",))
print(emitter.get_output())
