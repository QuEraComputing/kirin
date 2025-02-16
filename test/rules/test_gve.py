from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk
from kirin.rewrite.dce import DeadCodeElimination
from kirin.rewrite.gve import GlobalValueElimination


@basic_no_opt
def main_simplify_gv(x: int):
    y = 1
    z = 1
    h = 1
    return y + z + h + x


def test_gve():
    main_simplify_gv.print()
    assert len(main_simplify_gv.callable_region.blocks[0].stmts) == 7
    Walk(GlobalValueElimination()).rewrite(main_simplify_gv.code)
    Walk(DeadCodeElimination()).rewrite(main_simplify_gv.code)

    main_simplify_gv.print()

    assert len(main_simplify_gv.callable_region.blocks[0].stmts) == 5

    assert main_simplify_gv(2) == 5
