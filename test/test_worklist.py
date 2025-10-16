from kirin.worklist import WorkList


def test_worklist():
    wl = WorkList()

    assert wl.is_empty()
    assert not wl
    assert len(wl) == 0

    assert wl.pop() is None

    assert wl.is_empty()
    assert not wl
    assert len(wl) == 0

    wl.append("A")

    assert not wl.is_empty()
    assert wl
    assert len(wl) == 1

    assert wl.pop() == "A"

    assert wl.is_empty()
    assert not wl
    assert len(wl) == 0

    wl.append("Z")
    wl.extend("BCGFEDCB")

    assert not wl.is_empty()
    assert wl
    assert len(wl) == 9

    assert wl.pop() == "Z"
    assert wl.pop() == "B"

    assert not wl.is_empty()
    assert wl
    assert len(wl) == 7

    rest = []
    while wl:
        rest.append(wl.pop())
    assert rest == list("CGFEDCB")

    assert wl.is_empty()
    assert not wl
    assert len(wl) == 0
