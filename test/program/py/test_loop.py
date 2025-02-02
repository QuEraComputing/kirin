from kirin.prelude import basic_no_opt


def test_simple():
    @basic_no_opt
    def main(x: int):
        for i in range(5):
            x = x + 1
        return x

    assert main.py_func is not None
    assert main.py_func(1) == main(1)


# generate some more complicated loop
def test_nested():
    @basic_no_opt
    def main(x: int):
        for i in range(5):
            for j in range(5):
                x = x + 1
        return x

    assert main.py_func is not None
    assert main.py_func(1) == main(1)


def test_nested2():
    @basic_no_opt
    def main(x: int):
        for i in range(5):
            for j in range(5):
                for k in range(5):
                    x = x + 1
        return x

    assert main.py_func is not None
    assert main.py_func(1) == main(1)
