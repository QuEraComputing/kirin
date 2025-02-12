from kirin import ir, info, types, statement
from kirin.prelude import basic_no_opt
from kirin.analysis import const

dialect = ir.Dialect("mwe")


@statement(dialect=dialect)
class SideEffect(ir.Statement):
    name = "side_effect"
    traits = frozenset({ir.FromPythonCall()})
    value: ir.SSAValue = info.argument(types.Int)


@basic_no_opt.add(dialect)
def recursion(kernel, n: int, pos: int):
    if pos == n:
        return

    kernel(pos)
    recursion(kernel, n, pos + 1)


@basic_no_opt.add(dialect)
def side_effect(pos: int):
    SideEffect(pos)  # type: ignore


def test_non_pure_const():
    constprop = const.Propagate(basic_no_opt)
    frame, ret = constprop.run_analysis(
        recursion,
        (
            const.Value(side_effect),
            const.Result.top(),
            const.Result.top(),
        ),
    )
    # recursion.print(analysis=frame.entries)
    ret = frame.entries[recursion.callable_region.blocks[2].stmts.at(3).results[0]]
    assert isinstance(ret, const.Value)
    assert frame.frame_is_not_pure
