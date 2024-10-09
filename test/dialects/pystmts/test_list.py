from kirin.dialects.py import types
from kirin.prelude import basic


@basic(typeinfer=True)
def empty_list():
    return []


def test_empty_list():
    assert empty_list.return_type.is_equal(types.List[types.Any])
