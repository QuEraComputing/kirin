"""Regression tests for https://github.com/QuEraComputing/kirin/issues/706.

Deserialization used to assign `Statement._args` directly, bypassing the
`Statement.args` setter that registers `ir.Use(stmt, index)` on each operand.
The decoded IR therefore had correct forward operand references but empty
`SSAValue.uses` sets, which makes `DeadCodeElimination` (and hence `Fold`)
delete values that are still used.
"""

import pytest

from kirin import ir
from kirin.passes import Fold, HintConst
from kirin.prelude import basic, basic_no_opt
from kirin.dialects import ilist
from kirin.serialization.bsonserializer import CompressedBSONSerializer


@basic_no_opt
def straight_line(x: int) -> int:
    y = x + 1
    return y * 2


@basic_no_opt
def branching(x: int, flag: bool) -> int:
    if flag:
        y = x + 1
    else:
        y = x - 1
    return y * 2


@basic_no_opt
def looping(n: int) -> int:
    acc = 0
    for i in range(n):
        acc = acc + i
    return acc


@basic_no_opt
def ilist_consumer(x: int):
    values = ilist.IList([x, x + 1, x + 2])
    return values[0] + len(values)


@basic_no_opt
def nested_closure(y: int):
    def inner(x: int):
        return x * y + 1

    return inner(3)


KERNELS = [straight_line, branching, looping, ilist_consumer, nested_closure]
ARGS = {
    straight_line: (3,),
    branching: (3, True),
    looping: (5,),
    ilist_consumer: (7,),
    nested_closure: (4,),
}


def expected_uses(method: ir.Method) -> dict[ir.SSAValue, set[ir.Use]]:
    """The use sets implied by the forward operand references of `method`."""
    out: dict[ir.SSAValue, set[ir.Use]] = {}
    for stmt in method.code.walk():
        for index, operand in enumerate(stmt.args):
            out.setdefault(operand, set()).add(ir.Use(stmt, index))
    return out


def assert_use_def_consistent(method: ir.Method, label: str = "") -> None:
    """Every operand is mirrored by a use, and no use is stale or duplicated."""
    where = f"[{label}] " if label else ""
    expected = expected_uses(method)

    for value, uses in expected.items():
        for use in uses:
            assert (
                use in value.uses
            ), f"{where}missing reverse use {use.stmt.name}[{use.index}] on {value!r}"

    # the other direction: no spurious uses left over from decoding
    for stmt in method.code.walk():
        for value in list(stmt.args) + list(stmt.results):
            assert value.uses == expected.get(
                value, set()
            ), f"{where}unexpected uses on {value!r}: {value.uses}"


def round_trips(program: ir.Method):
    """Every decode path that goes through `BaseDeserializer`."""
    group = program.dialects
    yield "module", group.decode(group.encode(program))
    yield "json", group.decode_json(group.encode_json(program))

    bson = CompressedBSONSerializer()
    yield "bson", group.decode(bson.decode(bson.encode(group.encode(program))))


@pytest.mark.parametrize("kernel", KERNELS, ids=lambda k: k.sym_name)
def test_deserialized_uses_are_restored(kernel: ir.Method):
    # sanity check: the source method satisfies the invariant we assert on
    assert_use_def_consistent(kernel)

    for label, decoded in round_trips(kernel):
        assert_use_def_consistent(decoded, label)


@pytest.mark.parametrize("kernel", KERNELS, ids=lambda k: k.sym_name)
def test_fold_is_safe_after_round_trip(kernel: ir.Method):
    args = ARGS[kernel]
    expected = kernel(*args)

    for label, decoded in round_trips(kernel):
        Fold(decoded.dialects)(decoded)

        assert_use_def_consistent(decoded, label)
        decoded.verify()
        assert decoded(*args) == expected, label


def test_fold_after_hint_const_round_trip():
    """`hints` are intentionally not serialized; `HintConst` rebuilds them.

    With use-def links restored, folding a re-hinted method is well behaved.
    """

    @basic_no_opt
    def kernel() -> int:
        y = 1 + 2
        return y * 2

    decoded = basic_no_opt.decode_json(basic_no_opt.encode_json(kernel))
    HintConst(decoded.dialects)(decoded)
    Fold(decoded.dialects)(decoded)

    assert_use_def_consistent(decoded)
    decoded.verify()
    assert decoded() == kernel()


def test_use_indices_match_operand_positions():
    """A shared operand must record one use per position it appears at."""

    @basic
    def kernel(x: int) -> int:
        return x + x

    decoded = basic.decode_json(basic.encode_json(kernel))

    (add,) = (stmt for stmt in decoded.code.walk() if stmt.name == "add")
    lhs, rhs = add.args
    assert lhs is rhs
    assert lhs.uses == {ir.Use(add, 0), ir.Use(add, 1)}


def test_replace_by_reaches_decoded_uses():
    """Rewrites driven off `uses` must see the decoded statements."""

    @basic_no_opt
    def kernel(x: int) -> int:
        y = x + 1
        return y * 2

    decoded = basic_no_opt.decode_json(basic_no_opt.encode_json(kernel))

    (add,) = (stmt for stmt in decoded.code.walk() if stmt.name == "add")
    (mult,) = (stmt for stmt in decoded.code.walk() if stmt.name == "mult")
    x = add.args[0]

    # `replace_by` walks `uses`; with empty use sets it silently did nothing.
    add.results[0].replace_by(x)

    assert mult.args[0] is x
    assert not add.results[0].uses
    assert ir.Use(mult, 0) in x.uses
