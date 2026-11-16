from typing import Any, Union, Literal, Optional

from kirin import types
from kirin.prelude import basic
from kirin.dialects import ilist


def test_method_union_binop_hint():

    @basic
    def main(x: ilist.IList[float, Any] | list[float]) -> float:
        return x[0]

    main.print()

    tps = main.arg_types

    assert len(tps) == 1
    assert tps[0] == types.Union(
        [ilist.IListType[types.Float, types.Any], types.List[types.Float]]
    )


def test_method_union_multi_hint():

    @basic
    def main(x: str | float | int):
        return x

    main.print()

    tps = main.arg_types

    assert len(tps) == 1
    assert tps[0] == types.Union([types.String, types.Float, types.Int])


def test_hint2type_union_spellings():
    expected = types.Union([types.Int, types.NoneType])
    assert types.hint2type(Optional[int]) == expected
    assert types.hint2type(Union[int, None]) == expected
    assert types.hint2type(int | None) == expected


def test_method_optional_hint_inside_generic():
    # Python 3.14 makes `Optional[int]` a `types.UnionType`, like `int | None`;
    # both must keep the element type usable inside a generic hint.
    @basic
    def main(x: ilist.IList[Optional[int], Literal[2]]):
        return x

    (tp,) = main.arg_types
    assert (
        tp
        == ilist.IListType[types.Union([types.Int, types.NoneType]), types.Literal(2)]
    )
    assert ilist.IListType[types.Int, types.Literal(2)].is_subseteq(tp)
