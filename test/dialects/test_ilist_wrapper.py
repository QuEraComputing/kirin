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


def test_sorted():
    def key_test(a: int) -> int:
        return a

    @basic
    def main():
        ls = [2, 3, 1, 5, 4]
        return (
            ilist.sorted(ls),
            ilist.sorted(ls, key=key_test),
            ilist.sorted(ls, reverse=True),
        )

    main.print()

    ls1, ls2, ls3 = main()
    assert ls1.data == [1, 2, 3, 4, 5]
    assert ls2.data == ls1.data
    assert ls3.data == [5, 4, 3, 2, 1]

    def first(x: tuple[str, int]) -> str:
        return x[0]

    def second(x: tuple[str, int]) -> int:
        return x[1]

    @basic
    def main2():
        ls = [("a", 4), ("b", 3), ("c", 1)]
        return (
            ilist.sorted(ls, key=first),
            ilist.sorted(ls, key=second),
            ilist.sorted(ls, key=second, reverse=True),
        )

    main2.print()

    ls1, ls2, ls3 = main2()
    assert ls1.data == [("a", 4), ("b", 3), ("c", 1)]
    assert ls3.data == ls1.data
    assert ls2.data == [("c", 1), ("b", 3), ("a", 4)]
