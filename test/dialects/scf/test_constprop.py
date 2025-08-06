from typing import Any

from kirin.prelude import structural_no_opt
from kirin.analysis import const
from kirin.dialects import scf, func, ilist

prop = const.Propagate(structural_no_opt)


def test_simple_loop():
    @structural_no_opt
    def main():
        x = 0
        for i in range(2):
            x = x + 1
        return x

    frame, ret = prop.run_analysis(main)
    assert isinstance(ret, const.Value)
    assert ret.data == 2
    assert frame.frame_is_not_pure is False


def test_nested_loop():
    @structural_no_opt
    def main():
        x = 0
        for i in range(2):
            for j in range(3):
                x = x + 1
        return x

    frame, ret = prop.run_analysis(main)
    assert isinstance(ret, const.Value)
    assert ret.data == 6
    assert frame.frame_is_not_pure is False


def test_nested_loop_with_if():
    @structural_no_opt
    def main():
        x = 0
        for i in range(2):
            if i == 0:
                for j in range(3):
                    x = x + 1
        return x

    frame, ret = prop.run_analysis(main)
    assert isinstance(ret, const.Value)
    assert ret.data == 3
    assert frame.frame_is_not_pure is False


def test_nested_loop_with_if_else():
    @structural_no_opt
    def main():
        x = 0
        for i in range(2):
            if i == 0:
                for j in range(3):
                    x = x + 1
            else:
                for j in range(2):
                    x = x + 1
        return x

    frame, ret = prop.run_analysis(main)
    assert isinstance(ret, const.Value)
    assert ret.data == 5
    assert frame.frame_is_not_pure is False


def test_inside_return():
    @structural_no_opt
    def simple_loop(x: float):
        for i in range(0, 3):
            return i
        return x

    frame, ret = prop.run_analysis(simple_loop)
    assert isinstance(ret, const.Value)
    assert ret.data == 0

    # def test_simple_ifelse():
    @structural_no_opt
    def simple_ifelse(x: int):
        cond = x > 0
        if cond:
            return cond
        else:
            return 0

    simple_ifelse.print()
    frame, ret = prop.run_analysis(simple_ifelse)
    ifelse = simple_ifelse.callable_region.blocks[0].stmts.at(2)
    assert isinstance(ifelse, scf.IfElse)
    terminator = ifelse.then_body.blocks[0].last_stmt
    assert isinstance(terminator, func.Return)
    assert isinstance(frame.entries[terminator.value], const.Value)
    terminator = ifelse.else_body.blocks[0].last_stmt
    assert isinstance(terminator, func.Return)
    assert isinstance(value := frame.entries[terminator.value], const.Value)
    assert value.data == 0


def test_purity_1():

    @structural_no_opt
    def test_func(src: ilist.IList[float, Any]):

        def inner(i: int):
            if src[i] < 0:
                return 0.0
            elif src[i] < 1.0:
                return 1.0
            else:
                return 2.0

        return ilist.map(inner, ilist.range(len(src)))

    frame, ret = prop.run_analysis(test_func)

    assert not frame.frame_is_not_pure, "function should be pure"


def test_purity_2():

    @structural_no_opt
    def test_func(src: ilist.IList[float, Any]):

        def inner(i: int):
            value = 0.0
            if src[i] < 0:
                value = 0.0
            elif src[i] < 1.0:
                return 1.0
            else:
                value = 2.0

            return value

        return ilist.map(inner, ilist.range(len(src)))

    frame, ret = prop.run_analysis(test_func)

    assert not frame.frame_is_not_pure, "function should be pure"


def test_purity_3():

    @structural_no_opt
    def test_func(src: ilist.IList[float, Any]):

        def inner(i: int):
            value = 0.0
            if src[i] < 0:
                value = 0.0
            elif src[i] < 1.0:
                return 1.0
            else:
                return 2.0

            return value

        return ilist.map(inner, ilist.range(len(src)))

    frame, ret = prop.run_analysis(test_func)

    assert not frame.frame_is_not_pure, "function should be pure"


def test_purity_4():

    @structural_no_opt
    def test_func(src: list[float]):

        if True:
            return src
        else:
            src.append(2.0)
            return src

    frame, ret = prop.run_analysis(test_func)

    assert not frame.frame_is_not_pure, "function should be pure"


def test_purity_5():

    @structural_no_opt
    def test_func(src: list[float]):

        if False:
            src.append(2.0)

        return src

    frame, ret = prop.run_analysis(test_func)

    assert not frame.frame_is_not_pure, "function should be pure"


def test_purity_6():

    @structural_no_opt
    def test_func(src: list[float]):

        if True:
            return src
        else:
            src.append(2.0)

        return src

    frame, ret = prop.run_analysis(test_func)

    assert not frame.frame_is_not_pure, "function should be pure"


def test_purity_7():

    @structural_no_opt
    def test_func(src: list[float], cond: bool):

        if cond:
            src.append(2.0)
            return src
        else:
            return src

    frame, ret = prop.run_analysis(test_func)

    assert frame.frame_is_not_pure, "function should not be pure"


def test_purity_8():

    @structural_no_opt
    def test_func(src: ilist.IList[float, Any], dst: ilist.IList[float, Any]):
        assert len(src) == len(dst), "src and dst must have the same length"

        def inner(i: int):
            value = src[i]
            if src[i] < dst[i]:
                value = dst[i] - 3.0
            elif src[i] > dst[i]:
                return dst[i] + 3.0

            return value

        return ilist.map(inner, ilist.range(len(src)))

    frame, ret = prop.run_analysis(test_func)

    assert frame.frame_is_not_pure, "function should be pure"
