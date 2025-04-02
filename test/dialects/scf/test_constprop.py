from kirin.prelude import structural_no_opt
from kirin.analysis import const
from kirin.dialects import scf, func

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
