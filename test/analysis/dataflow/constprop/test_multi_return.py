from kirin import ir, types, lowering
from kirin.decl import info, statement
from kirin.passes import Fold, TypeInfer
from kirin.prelude import structural_no_opt
from kirin.analysis import const
from kirin.dialects import py, func, ilist

dialect = ir.Dialect("analysis")


@statement(dialect=dialect)
class MultiReturnStatement(ir.Statement):
    traits = frozenset({lowering.FromPythonCall(), ir.Pure()})
    inputs: tuple[ir.SSAValue] = info.argument(types.Any)

    def __init__(self, *args: ir.SSAValue):
        super().__init__(
            args=args,
            result_types=tuple(arg.type for arg in args),
            args_slice={"inputs": slice(None)},
        )


@ir.dialect_group(structural_no_opt.add(dialect))
def dialect_group_test(self):
    fold = Fold(self)
    type_infer = TypeInfer(self)

    def run_pass(mt):
        type_infer(mt)
        fold(mt)

    return run_pass


def test_multi_return_default_prop():

    stmts = [
        (a := py.Constant(1)),
        (b := py.Constant(2)),
        (res := MultiReturnStatement(a.result, b.result)),
        (return_result := ilist.New((res.results[0], res.results[1]))),
        (func.Return(return_result.result)),
    ]

    body = ir.Region(ir.Block(stmts))
    func_code = func.Function(
        sym_name="test", signature=func.Signature((), types.Any), body=body
    )
    mt = ir.Method(None, None, "test", (), dialect_group_test, func_code)

    frame, return_result = const.Propagate(dialect_group_test).run_analysis(
        mt, no_raise=False
    )

    assert frame.entries[res.results[0]] == const.Unknown()
    assert frame.entries[res.results[1]] == const.Unknown()
