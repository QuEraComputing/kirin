import pytest

from kirin.passes import Specialize
from kirin.prelude import basic_no_opt
from kirin.dialects import func, debug, ilist
from kirin.dialects.py.constant import Constant

kernel = basic_no_opt.add(debug)


def test_map_nested_callback_specialization(capsys):
    @kernel
    def mapped_local(n: int):
        def round_body(i: int):
            parity = i % 2
            debug.info("round parity", parity)
            return parity

        return ilist.map(round_body, ilist.range(n))

    @kernel
    def root():
        return mapped_local(4)

    # The length becomes known only inside a newly specialized method.
    Specialize(root.dialects, no_raise=False)(root)
    (owner_call,) = [s for s in root.code.walk() if isinstance(s, func.Invoke)]
    body = owner_call.callee
    assert not any(isinstance(s, ilist.Map) for s in body.code.walk())
    calls = [s for s in body.code.walk() if isinstance(s, func.Invoke)]
    assert len(calls) == 4
    for parity, call in enumerate(calls):
        assert not call.inputs
        (log,) = [s for s in call.callee.code.walk() if isinstance(s, debug.Info)]
        value = log.inputs[0].owner
        assert isinstance(value, Constant)
        assert value.value.data == parity % 2
        call.callee.verify()
    assert capsys.readouterr().out == ""  # Compilation must not emit effects.
    assert list(root()) == [0, 1, 0, 1]
    output = capsys.readouterr().out
    assert output.count("INFO:") == 4
    assert output.index("parity = 0") < output.index("parity = 1")


def test_map_unroll_above_former_length_limit(capsys):
    values = ilist.IList([7] * 64)

    @kernel
    def visit(i: int):
        debug.info("element", i)
        return i

    @kernel
    def root():
        return ilist.map(visit, values)

    Specialize(root.dialects, no_raise=False)(root)
    assert not any(isinstance(s, ilist.Map) for s in root.code.walk())
    calls = [s for s in root.code.walk() if isinstance(s, func.Invoke)]
    assert len(calls) == 64
    assert all(not call.inputs for call in calls)
    assert len({call.callee for call in calls}) == 1
    root.verify()
    assert capsys.readouterr().out == ""
    assert list(root()) == [7] * 64
    assert capsys.readouterr().out.count("INFO:") == 64


def test_map_partially_constant_elements(capsys):
    @kernel
    def visit(i: int):
        debug.info("element", i)
        return i + 1

    @kernel
    def root(a: int):
        xs = [1, 2, a, 4]
        return ilist.map(visit, xs)

    Specialize(root.dialects, no_raise=False)(root)
    assert not any(isinstance(s, ilist.Map) for s in root.code.walk())
    calls = [s for s in root.code.walk() if isinstance(s, func.Invoke)]
    assert len(calls) == 4
    # The three literal elements bind; the runtime one stays a real argument.
    bound = [call for call in calls if not call.inputs]
    assert len(bound) == 3
    assert all("_specialized_" in call.callee.sym_name for call in bound)
    (generic,) = [call for call in calls if call.inputs]
    assert generic.callee is visit
    root.verify()
    assert capsys.readouterr().out == ""  # Compilation must not emit effects.
    assert list(root(99)) == [2, 3, 100, 5]
    assert capsys.readouterr().out.count("INFO:") == 4


@pytest.mark.parametrize("operation", ["map", "foldl"])
def test_dynamic_elements_remain_rolled(operation, capsys):
    @kernel
    def visit(i: int):
        debug.info("element", i)
        return i

    @kernel
    def step(acc: int, i: int):
        debug.info("element", i)
        return acc + i

    @kernel
    def mapped(a: int):
        return ilist.map(visit, [a, a])

    # foldl also pins that a constant init alone does not justify expansion.
    @kernel
    def foldl(a: int):
        return ilist.foldl(step, [a, a], 0)

    root, statement = {
        "map": (mapped, ilist.Map),
        "foldl": (foldl, ilist.Foldl),
    }[operation]
    Specialize(root.dialects, no_raise=False)(root)
    assert any(isinstance(s, statement) for s in root.code.walk())
    root.verify()
    assert capsys.readouterr().out == ""
    root(7)
    assert capsys.readouterr().out.count("INFO:") == 2


@pytest.mark.parametrize("supported_element", [False, True])
def test_constant_elements_require_supported_keys(supported_element, capsys):
    unsupported = object()
    values = ilist.IList([unsupported, 1 if supported_element else unsupported])

    @kernel
    def visit(value):
        debug.info("element", value)
        return value

    @kernel
    def root():
        return ilist.map(visit, values)

    Specialize(root.dialects, no_raise=False)(root)
    assert any(isinstance(s, ilist.Map) for s in root.code.walk()) != supported_element
    if supported_element:
        calls = [s for s in root.code.walk() if isinstance(s, func.Invoke)]
        assert len(calls) == 2
        assert len(calls[0].inputs) == 1
        assert not calls[1].inputs
    root.verify()
    assert capsys.readouterr().out == ""
    result = list(root())
    assert result[0] is unsupported
    if supported_element:
        assert result[1] == 1
    else:
        assert result[1] is unsupported
    assert capsys.readouterr().out.count("INFO:") == 2


@pytest.mark.parametrize("operation", ["map", "for_each", "foldl", "foldr", "scan"])
def test_unroll_preserves_results_and_effect_order(operation, capsys):
    @kernel
    def visit(i: int):
        debug.info("element", i)
        return i + 1

    @kernel
    def step(acc: int, i: int):
        debug.info("element", i)
        return acc * 2 + i

    @kernel
    def scan_step(acc: int, i: int):
        debug.info("element", i)
        updated = acc * 2 + i
        return updated, updated

    @kernel
    def mapped(initial: int):
        return ilist.map(visit, ilist.range(12))

    @kernel
    def for_each(initial: int):
        ilist.for_each(visit, ilist.range(12))
        return initial

    @kernel
    def foldl(initial: int):
        return ilist.foldl(step, ilist.range(12), initial)

    @kernel
    def foldr(initial: int):
        return ilist.foldr(step, ilist.range(12), initial)

    @kernel
    def scan(initial: int):
        return ilist.scan(scan_step, ilist.range(12), initial)

    root, kind = {
        "map": (mapped, ilist.Map),
        "for_each": (for_each, ilist.ForEach),
        "foldl": (foldl, ilist.Foldl),
        "foldr": (foldr, ilist.Foldr),
        "scan": (scan, ilist.Scan),
    }[operation]
    expected = root(3)
    expected_output = capsys.readouterr().out
    Specialize(root.dialects, no_raise=False)(root)
    assert capsys.readouterr().out == ""
    calls = [s for s in root.code.walk() if isinstance(s, func.Invoke)]
    assert len(calls) == 12
    remainder = [s for s in root.code.walk() if isinstance(s, kind)]
    assert not remainder
    for offset, call in enumerate(calls):
        assert len(call.inputs) == (1 if operation in ("foldl", "foldr", "scan") else 0)
        (log,) = [s for s in call.callee.code.walk() if isinstance(s, debug.Info)]
        assert isinstance(log.inputs[0].owner, Constant)
        assert log.inputs[0].owner.value.data == (
            11 - offset if operation == "foldr" else offset
        )
    root.verify()
    actual = root(3)
    if operation == "map":
        assert list(actual) == list(expected)
    elif operation == "scan":
        assert actual[0] == expected[0]
        assert list(actual[1]) == list(expected[1])
    else:
        assert actual == expected
    assert capsys.readouterr().out == expected_output
