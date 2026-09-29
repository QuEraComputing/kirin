from kirin import ir, types
from kirin.decl import info, statement
from kirin.passes import TypeInfer
from kirin.prelude import basic_no_opt
from kirin.dialects import py, func

dialect = ir.Dialect("demo")


@statement(dialect=dialect)
class Apply(ir.Statement):
    """Two variadic argument groups of different types around a scalar argument."""

    name = "apply"
    targets: tuple[ir.SSAValue, ...] = info.argument(types.Int)
    flag: ir.SSAValue = info.argument(types.Bool)
    params: tuple[ir.SSAValue, ...] = info.argument(types.Float)


def test_fallback_pairs_each_argument_with_its_own_field():
    target, second = py.Constant(1), py.Constant(2)
    flag, angle = py.Constant(True), py.Constant(0.5)
    apply = Apply(
        targets=(target.result, second.result),
        flag=flag.result,
        params=(angle.result,),
    )
    none = func.ConstantNone()
    block = ir.Block(
        [target, second, flag, angle, apply, none, func.Return(none.result)]
    )
    block.args.append_from(types.Any, "self")
    code = func.Function(
        sym_name="main",
        signature=func.Signature(inputs=(), output=types.NoneType),
        body=ir.Region(block),
    )
    group = basic_no_opt.add(dialect)
    TypeInfer(group)(
        ir.Method(dialects=group, code=code, sym_name="main", arg_names=[])
    )
    assert (target.result.type, second.result.type) == (types.Int, types.Int)
    assert flag.result.type == types.Bool
    assert angle.result.type == types.Float
