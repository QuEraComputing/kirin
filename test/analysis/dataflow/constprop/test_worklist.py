from kirin.prelude import basic_no_opt
from kirin.analysis import const


def test_worklist_bfs():
    @basic_no_opt
    def make_ker(val: float):

        def ker(i: float):
            return i + val

        return ker

    @basic_no_opt
    def test(x: str, y: float, flag: bool):

        if x == "x":
            val = 1.0
        else:
            val = 2.0

        if flag:
            ker = make_ker(val=val)

        else:
            ker = make_ker(val=val)

        return ker

    # test.print()
    prop = const.Propagate(basic_no_opt)
    frame, ret = prop.run_analysis(test)
    assert isinstance(ret, const.PartialLambda)
