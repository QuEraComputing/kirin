from kirin.prelude import basic_no_opt
from kirin.dialects import debug


def test_debug_printing():
    @basic_no_opt.add(debug)
    def test_if_inside_for() -> int:
        count = 0
        for i in range(5):
            count = count + 1
            something_else = count + 2
            debug.info("current count before", count, something_else)
            if True:
                count = count + 100
                debug.info("inside the ifelse", count, something_else)
            else:
                count = count + 300
        return count

    test_if_inside_for()
