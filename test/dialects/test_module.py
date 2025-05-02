import pytest

from kirin import ir, types
from kirin.prelude import basic
from kirin.dialects import py, func, module


def test_module():
    fn1 = func.Function(
        sym_name="foo",
        slots=(),
        body=ir.Region(
            ir.Block(
                [
                    x := py.Constant(1),
                    func.Return(x.result),
                ],
                argtypes=(types.Any,),
            )
        ),
        signature=func.Signature(inputs=(), output=types.Int),
    )

    fn2 = func.Function(
        sym_name="main",
        slots=(),
        body=ir.Region(
            ir.Block(
                [
                    x := module.Invoke((), (), callee="foo"),
                    func.Return(x.result),
                ],
                argtypes=(types.Any,),
            )
        ),
        signature=func.Signature(inputs=(), output=types.Int),
    )

    mod = module.Module(
        sym_name="test_module", entry="main", body=ir.Region(ir.Block([fn1, fn2]))
    )

    dialects = basic.add(module)
    method = ir.Method(dialects=dialects, code=mod)
    method.print()

    with pytest.raises(KeyError):
        method()

    dialects.update_symbol_table(method)
    assert method() == 1
