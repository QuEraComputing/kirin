from kirin.prelude import basic
from kirin.dialects import ilist


def test_map_wrapper():

    @basic
    def add1(x: int):
        return x + 1

    @basic
    def map_wrap():
        return ilist.map(add1, range(5))

    out = map_wrap()
    assert isinstance(out, ilist.IList)
    assert out.data == [1, 2, 3, 4, 5]


def test_foldr_wrapper():

    @basic
    def add_fold(x: int, out: int):
        return out + x

    @basic
    def map_foldr():
        return ilist.foldr(add_fold, range(5), init=10)

    out = map_foldr()
    assert isinstance(out, int)
    assert out == 10 + 0 + 1 + 2 + 3 + 4


def test_foldl_wrapper():

    @basic
    def add_fold2(out: int, x: int):
        return out + x

    @basic
    def map_foldl():
        return ilist.foldr(add_fold2, range(5), init=10)

    out = map_foldl()
    assert isinstance(out, int)
    assert out == 10 + 0 + 1 + 2 + 3 + 4


def test_scan_wrapper():

    @basic
    def add_scan(out: int, x: int):
        return out + 1, out + x

    @basic
    def scan_wrap():
        return ilist.scan(add_scan, range(5), init=10)

    out = scan_wrap()
    assert isinstance(out, tuple)
    assert len(out) == 2

    res = out[0]
    out_list = out[1]

    assert isinstance(res, int)
    assert res == 10 + 1 * 5

    assert isinstance(out_list, ilist.IList)
    assert out_list.data == [
        10 + 0,
        10 + 1 + 1,
        10 + 1 + 1 + 2,
        10 + 1 + 1 + 1 + 3,
        10 + 1 + 1 + 1 + 1 + 4,
    ]


def test_any_all_wrapper():

    @basic
    def test_any_all():
        ls = [True, False, False]
        return ls, ilist.any(ls), ilist.all(ls)

    test_any_all.print()

    ls, any_val, all_val = test_any_all()

    assert isinstance(ls, ilist.IList)
    assert ls.data == [True, False, False]
    assert any_val
    assert not all_val

    @basic
    def test_any_all2():
        ls = [False, False]
        return ilist.any(ls), ilist.all(ls)

    any_val, all_val = test_any_all2()
    assert not any_val
    assert not all_val

    @basic
    def test_any_all3():
        ls = [True, True, True, True, True]
        return ilist.any(ls), ilist.all(ls)

    any_val, all_val = test_any_all3()
    assert any_val
    assert all_val
