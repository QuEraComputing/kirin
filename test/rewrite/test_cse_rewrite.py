from kirin.prelude import basic_no_opt
from kirin.rewrite.cse import CommonSubexpressionElimination
from kirin.rewrite.walk import Walk


@basic_no_opt
def main():
    x = 1
    y = 1
    z = 1
    return x + y + z


main.print()

# the bug only really rears its head if there's multiple things in the ._result attribute of the statement
# otherwise it's perfectly valid to handle what just happened
result = Walk(CommonSubexpressionElimination()).rewrite(main.code)

main.print()
