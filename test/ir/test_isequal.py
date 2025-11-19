from kirin import ir, types
from kirin.dialects import func


def test_is_structurally_equal_ignoring_hint():
    block = ir.Block()
    block.args.append_from(types.MethodType, "self")
    source_func = func.Function(
        sym_name="main",
        signature=func.Signature(
            inputs=(),
            output=types.NoneType,
        ),
        body=ir.Region(block),
    )

    block = ir.Block()
    block.args.append_from(types.MethodType, "self")
    expected_func = func.Function(
        sym_name="main",
        signature=func.Signature(
            inputs=(),
            output=types.NoneType,
        ),
        body=ir.Region(block),
    )

    assert expected_func.is_structurally_equal(source_func)
