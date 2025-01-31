from dataclasses import dataclass

from kirin.prelude import basic


def test_getattr():
    @dataclass
    class MyFoo:
        x: float
        y: float
        z: float

    foo = MyFoo(1.0, 2.0, 3.0)

    @basic
    def main():
        return foo.x + 1.0

    main.print()
    out = main()

    assert out == 2.0
