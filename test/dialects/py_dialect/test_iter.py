import kirin.prelude


def iter_non_pure():
    @kirin.prelude.basic
    def loop(a: str):
        out = []
        for i in range(4):
            # for i in [1,1,2,3]:  # Same result with this line instead
            out = out + [a]
        return out

    x = loop("a")
    assert x == ["a", "a", "a", "a"]

    x = loop("b")
    assert x == ["b", "b", "b", "b"]

    x = loop("c")
    assert x == ["c", "c", "c", "c"]
