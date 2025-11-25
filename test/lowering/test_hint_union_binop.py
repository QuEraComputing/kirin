from typing import Any

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
