from kirin import types
from kirin.prelude import basic, basic_no_opt
from kirin.analysis import TypeInference
from kirin.dialects import py, func


@basic_no_opt
def main(x):
    y: int = x
    return y


def test_ann_assign():
    stmt = main.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, py.assign.TypeAssert)

    typeinfer = TypeInference(basic_no_opt)
    _, ret = typeinfer.run_analysis(main, (types.Int,))
    assert ret.is_equal(types.Int)
    _, ret = typeinfer.run_analysis(main, (types.Float,))
    assert ret is ret.bottom()


def test_typeinfer_simplify_assert():
    @basic(typeinfer=True, fold=False)
    def simplify(x: int):
        y: int = x
        return y

    stmt = simplify.callable_region.blocks[0].stmts.at(0)
    assert isinstance(stmt, func.Return)
