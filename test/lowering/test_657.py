from kirin.prelude import basic
from kirin.dialects import ilist


def test_657():

    @basic
    def main():
        return 3

    @basic
    def main2():
        return 4

    @basic
    def main_final():
        return [main, main2]

    assert main_final() == ilist.IList([main, main2])
