from typing import Any

from kirin import types
from kirin.prelude import basic
from kirin.dialects.ilist import IList, IListType


@basic
def complicated_type(x: IList[tuple[float, float, IList[float, Any]], Any]):
    return x


def test_complicated_type():
    typ = complicated_type.arg_types[0]
    assert isinstance(typ, types.Generic)
    assert typ.is_subseteq(
        IListType[
            types.Tuple[types.Float, types.Float, IListType[types.Float]], types.Any
        ]
    )
