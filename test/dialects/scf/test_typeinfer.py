from kirin import types
from kirin.prelude import structural_no_opt
from kirin.analysis import TypeInference
from kirin.dialects import ilist

type_infer = TypeInference(structural_no_opt)


def test_inside_return_loop():
    @structural_no_opt
    def simple_loop(x: float):
        for i in range(0, 3):
            return i
        return x

    frame, ret = type_infer.run_analysis(simple_loop)
    assert ret.is_subseteq(types.Int | types.Float)


def test_simple_ifelse():
    @structural_no_opt
    def simple_ifelse(x: int):
        cond = x > 0
        if cond:
            return cond
        else:
            return 0

    frame, ret = type_infer.run_analysis(simple_ifelse)
    assert ret.is_subseteq(types.Bool | types.Int | types.NoneType)


def test_getitem_inside_loop():
    @structural_no_opt
    def getitem_loop():
        ls = [1, 2, 3]
        for i in range(2):
            ls[i + 1]

    frame, ret = type_infer.run_analysis(getitem_loop)

    getitem_loop.print(analysis=frame.entries)

    # NOTE: go through values, but skip method and return
    for value in list(frame.entries.values())[1:-1]:
        # everything except the method & return should either be int or a list[int]
        print(value)
        print(value.is_subseteq(types.Int | ilist.IListType[types.Int, types.Any]))

    assert ret.is_subseteq(types.NoneType)

    @structural_no_opt
    def getitem_return_from_loop():
        ls = [1, 2, 3]
        for i in range(2):
            return ls[i + 1]

        return 1.0

    frame, ret = type_infer.run_analysis(getitem_return_from_loop)
    assert ret.is_subseteq(types.Int | types.Float)
