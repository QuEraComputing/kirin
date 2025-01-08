from kirin.prelude import basic
from kirin.rewrite import Walk
from kirin.analysis import const
from kirin.dialects import fcf
from kirin.analysis.const.prop import Propagate
from kirin.dialects.fcf.rewrite.fcfmap_inline import InlineFcfMap


def test_fcfmap_rewrite():

    @basic(fold=False)
    def fcf_map_rewrite():

        def _simple(i: int):
            return i

        tmp = fcf.Map(_simple, range(5))
        return tmp

    fcf_map_rewrite.code.print()
    cp = Propagate(dialects=fcf_map_rewrite.dialects)
    cp.eval(fcf_map_rewrite, ())
    Walk(InlineFcfMap(cp.results)).rewrite(fcf_map_rewrite.code)
    fcf_map_rewrite.code.print()

    val = fcf_map_rewrite()

    assert val == (0, 1, 2, 3, 4)


def test_fcfmap_rewrite_with_arg():

    @basic(fold=False)
    def fcf_map_rewrite_with_arg(x: int):

        def _simple(i: int):
            return i

        tmp = fcf.Map(_simple, (x, x + 1, x + 2))
        return tmp

    fcf_map_rewrite_with_arg.code.print()
    cp = Propagate(dialects=fcf_map_rewrite_with_arg.dialects)
    cp.eval(fcf_map_rewrite_with_arg, (const.JointResult.top(),))
    Walk(InlineFcfMap(cp.results)).rewrite(fcf_map_rewrite_with_arg.code)
    fcf_map_rewrite_with_arg.code.print()

    val = fcf_map_rewrite_with_arg(4)
    print(val)
    assert val == (4, 5, 6)


test_fcfmap_rewrite_with_arg()
