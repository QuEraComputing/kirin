from stmts import Pour, Puke, Drink, NewBeer
from recept import FeeAnalysis
from dialect import dialect

from interp import BeerMethods as BeerMethods
from lattice import AtLeastXItem
from rewrite import RandomWalkBranch
from kirin.ir import dialect_group
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint
from kirin.passes.fold import Fold


# create our own beer dialect, it runs a random walk on the branches
@dialect_group(basic_no_opt.add(dialect))
def beer(self):

    fold_pass = Fold(self)

    def run_pass(mt, *, fold=True):
        Fixpoint(Walk(RandomWalkBranch())).rewrite(mt.code)

        # add const fold
        if fold:
            fold_pass(mt)

    return run_pass


# we are going to get drunk!
# add our beer dialect to the default dialect (builtin, cf, func, ...)


# type: ignore
@beer
def main(x: int):
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


main.code.print()
main(1)  # execute the function
# for i in range(10):
#     print("iteration", i)
#     main(i)  # now drink a random beer!


# simple analysis example:
@beer
def main2(x: int):

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


fee_analysis = FeeAnalysis(main2.dialects)
results, expect = fee_analysis.run_analysis(main2, args=(AtLeastXItem(data=10),))
print(results)
print(fee_analysis.puke_count)
main2.print(analysis=results)
