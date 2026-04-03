from pytest import mark

from kirin import ir, lowering
from kirin.decl import statement
from kirin.prelude import structural_no_opt
from kirin.analysis import const
from kirin.dialects import scf, func

prop = const.Propagate(structural_no_opt)

# A statement with no Pure/MaybePure trait — acts as a side effect.
_impure_dialect = ir.Dialect("test_impure")


@statement(dialect=_impure_dialect)
class ImpureOp(ir.Statement):
    name = "impure_op"
    traits = frozenset({lowering.FromPythonCall()})


def test_simple_loop():
    @structural_no_opt
    def main():
        x = 0
        for i in range(2):
            x = x + 1
        return x

    frame, ret = prop.run(main)
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

    frame, ret = prop.run(main)
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

    frame, ret = prop.run(main)
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

    frame, ret = prop.run(main)
    assert isinstance(ret, const.Value)
    assert ret.data == 5
    assert frame.frame_is_not_pure is False


@mark.xfail(reason="if with early return not supported in scf lowering")
def test_inside_return():
    @structural_no_opt
    def simple_loop(x: float):
        for i in range(0, 3):
            return i
        return x

    frame, ret = prop.run(simple_loop)
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
    frame, ret = prop.run(simple_ifelse)
    ifelse = simple_ifelse.callable_region.blocks[0].stmts.at(2)
    assert isinstance(ifelse, scf.IfElse)
    terminator = ifelse.then_body.blocks[0].last_stmt
    assert isinstance(terminator, func.Return)
    assert isinstance(frame.entries[terminator.value], const.Value)
    terminator = ifelse.else_body.blocks[0].last_stmt
    assert isinstance(terminator, func.Return)
    assert isinstance(value := frame.entries[terminator.value], const.Value)
    assert value.data == 0


def test_early_termination_when_body_ignores_iter_var():
    """When the body doesn't reference the iteration variable and loop_vars
    converge (x is Unknown, so Unknown + 1 = Unknown), early termination
    should fire and produce the same result as running all iterations."""

    @structural_no_opt
    def converging_loop(x: int) -> int:
        for _i in range(100):
            x = x + 1
        return x

    constprop = const.Propagate(structural_no_opt)
    frame, ret = constprop.run(converging_loop)

    assert isinstance(ret, const.Unknown)
    [for_stmt] = [s for s in converging_loop.code.walk() if isinstance(s, scf.For)]
    assert for_stmt in frame.should_be_pure


def test_no_early_termination_when_body_uses_iter_var():
    """Early termination must not fire when the body references the iteration
    variable, because later iterations may follow different code paths that
    affect purity.  Here the impure ``ImpureOp`` is guarded by ``i == 50``,
    so the loop body is impure only on iteration 50.  If early termination
    incorrectly broke after iteration 1 (where loop_vars converge), the
    for-loop would be marked as pure when it is not."""

    _group = structural_no_opt.add(_impure_dialect)

    @_group
    def impure_on_later_iter(x: int) -> int:
        for i in range(100):
            if i == 50:
                ImpureOp()
            x = x + 1
        return x

    constprop = const.Propagate(_group)
    frame, ret = constprop.run(impure_on_later_iter)

    [for_stmt] = [s for s in impure_on_later_iter.code.walk() if isinstance(s, scf.For)]
    # The for-loop must NOT be in should_be_pure — it contains a
    # conditionally-impure operation on a later iteration.
    assert for_stmt not in frame.should_be_pure
