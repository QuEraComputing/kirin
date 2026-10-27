from kirin import types
from kirin.prelude import basic
from kirin.dialects import ilist


def test_push_one_element():
    @basic(typeinfer=True)
    def push_one(xs: ilist.IList[int, types.Any]):
        return ilist.Push(xs, 4)

    push_one.verify_type()
    assert push_one.return_type.is_subseteq(ilist.IListType[types.Int, types.Any])
    assert push_one(ilist.IList([1, 2, 3], elem=types.Int)) == ilist.IList(
        [1, 2, 3, 4], elem=types.Int
    )


def test_push_onto_list_of_any():
    @basic(typeinfer=True)
    def push_one(xs: ilist.IList[types.Any, types.Any]):
        return ilist.Push(xs, 4)

    push_one.verify_type()
    assert push_one.return_type == ilist.IListType[types.Any, types.Any]


def test_push_wrong_element_type():
    @basic(typeinfer=True)
    def push_one(xs: ilist.IList[int, types.Any]):
        return ilist.Push(xs, 4.0)

    assert push_one.return_type == types.Bottom
