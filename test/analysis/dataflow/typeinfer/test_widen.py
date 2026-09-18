from pytest import mark

from kirin import types
from kirin.dialects.ilist import IListType
from kirin.analysis.typeinfer.widen import widen


@mark.parametrize(
    "previous,current,expected",
    [
        (types.Bottom, types.Int, types.Int),
        (types.Any, types.Int, types.Any),
        (types.Int, types.Float, types.Int | types.Float),
        (
            IListType[types.Int, types.Literal(1)],
            IListType[types.Float, types.Literal(1)],
            IListType[types.Int | types.Float, types.Literal(1)],
        ),
        (
            IListType[types.Int, types.Literal(0)],
            IListType[types.Int, types.Literal(1)],
            IListType[types.Int, types.Any],
        ),
        (
            types.Tuple[IListType[types.Int, types.Literal(0)], types.Bool],
            types.Tuple[IListType[types.Int, types.Literal(1)], types.Bool],
            types.Tuple[IListType[types.Int, types.Any], types.Bool],
        ),
    ],
)
def test_widen_preserves_upper_bounds(previous, current, expected):
    result = widen(previous, current)
    assert result == expected
    assert previous.is_subseteq(result)
    assert current.is_subseteq(result)
    assert widen(result, current) == result
