from kirin.prelude import structural_no_opt
from kirin.analysis import const

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
