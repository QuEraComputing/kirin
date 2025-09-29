import random

import kirin.prelude
from kirin.dialects import ilist, random as kirin_random


def test_random():

    random.seed(12)
    expected = [random.random() for _ in range(4)]

    @kirin.prelude.basic
    def rnd_main() -> ilist.IList:
        kirin_random.seed(12)
        out = []
        for i in range(4):
            # for i in [1,1,2,3]:  # Same result with this line instead
            out = out + [kirin_random.random()]
        return out

    out: ilist.IList = rnd_main()

    assert out.data == expected


def test_randint():

    random.seed(12)
    expected = [random.randint(i, 10) for i in range(4)]

    @kirin.prelude.basic
    def rndint_main() -> ilist.IList:
        kirin_random.seed(12)
        out = []
        for i in range(4):
            out = out + [kirin_random.randint(i, 10)]
        return out

    out: ilist.IList = rndint_main()

    assert out.data == expected


def test_uniform():

    random.seed(12)
    expected = [random.uniform(i, 10) for i in range(4)]

    @kirin.prelude.basic
    def rnduniform_main() -> ilist.IList:
        kirin_random.seed(12)
        out = []
        for i in range(4):
            out = out + [kirin_random.uniform(i, 10)]
        return out

    out: ilist.IList = rnduniform_main()

    assert out.data == expected
