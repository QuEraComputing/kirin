from stmts import Pour, Puke, Drink, NewBeer
from dialect import dialect

from interp import BeerInterpreter as BeerInterpreter
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
    beer = NewBeer("budlight")
    Drink(beer)
    Pour(beer, 12)
    Puke()
    if x > 1:
        Drink(NewBeer("heineken"))
    else:
        Drink(NewBeer("tsingdao"))
    return x + 1


main.code.print()
main(1)  # execute the function

# for i in range(10):
#     print("iteration", i)
#     main(i)  # now drink a random beer!
