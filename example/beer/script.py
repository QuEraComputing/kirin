from stmts import Pour, Puke, Drink, NewBeer
from dialect import dialect

from interp import BeerMethods as BeerMethods
from rewrite import RandomWalkBranch
from kirin.ir import dialect_group
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk, Fixpoint


# create our own beer dialect, it runs a random walk on the branches
@dialect_group(basic_no_opt.add(dialect))
def beer(self):
    def run_pass(mt):
        Fixpoint(Walk(RandomWalkBranch())).rewrite(mt.code)

    return run_pass


# we are going to get drunk!
# add our beer dialect to the default dialect (builtin, cf, func, ...)


# type: ignore
@beer
def main(x):
    def some_closure(beer, amount):
        Pour(beer, amount + 1)
        Puke()

    beer = NewBeer(brand="budlight")
    Drink(beer)
    Pour(beer, 12 + x)
    Puke()
    some_closure(beer, 1 + 1)
    if x > 1:
        Drink(NewBeer(brand="heineken"))
    else:
        Drink(NewBeer(brand="tsingdao"))
    return x + 1


main.code.print()
main(1)  # execute the function
# for i in range(10):
#     print("iteration", i)
#     main(i)  # now drink a random beer!


# analysis:
from recept import FeeAnalysis

from lattice import NotItem
from kirin.analysis.const import Value, Propagate, JointResult

cp = Propagate(main.dialects)
cp_results, expect = cp.run_analysis(main, (JointResult.from_const(1),))
print(cp_results)

fee_analysis = FeeAnalysis(main.dialects, constprop_results=cp_results)
results, expect = fee_analysis.run_analysis(main, args=(NotItem(),))
print(results)
