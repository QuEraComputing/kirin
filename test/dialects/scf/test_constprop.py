from kirin import ir
from kirin.prelude import python_basic
from kirin.analysis import const
from kirin.dialects import py, scf, func, ilist, lowering


@ir.dialect_group(
    python_basic.union([py.range, ilist, scf, func, lowering.func, lowering.call])
)
def kernel(self):
    def run_pass(method):
        pass

    return run_pass


def test_simple_loop():
    @kernel
    def main():
        x = 0
        for i in range(2):
            x = x + 1
        return x

    prop = const.Propagate(kernel)
    result, ret = prop.run_analysis(main)
    assert isinstance(ret.const, const.Value)
    assert ret.const.data == 2
    assert isinstance(ret.purity, const.Pure)


def test_nested_loop():
    @kernel
    def main():
        x = 0
        for i in range(2):
            for j in range(3):
                x = x + 1
        return x

    prop = const.Propagate(kernel)
    result, ret = prop.run_analysis(main)
    assert isinstance(ret.const, const.Value)
    assert ret.const.data == 6
    assert isinstance(ret.purity, const.Pure)


def test_nested_loop_with_if():
    @kernel
    def main():
        x = 0
        for i in range(2):
            if i == 0:
                for j in range(3):
                    x = x + 1
        return x

    prop = const.Propagate(kernel)
    result, ret = prop.run_analysis(main)
    assert isinstance(ret.const, const.Value)
    assert ret.const.data == 3
    assert isinstance(ret.purity, const.Pure)


def test_nested_loop_with_if_else():
    @kernel
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

    prop = const.Propagate(kernel)
    result, ret = prop.run_analysis(main)
    assert isinstance(ret.const, const.Value)
    assert ret.const.data == 5
    assert isinstance(ret.purity, const.Pure)
