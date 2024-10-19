from kirin.dialects.py import types
from kirin.prelude import basic


@basic(typeinfer=True)
def empty_list():
    return []


def test_empty_list():
    assert empty_list.return_type.is_subseteq(types.List[types.Any])


def test_list_len():
    @basic(typeinfer=True)
    def list_len(lst: list):
        return len(lst)

    assert list_len.return_type.is_equal(types.Int)
