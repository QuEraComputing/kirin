from kirin.passes import TypeInfer
from kirin.prelude import basic_no_opt


def test_always_rewrites():
    @basic_no_opt
    def unstable(x: int):  # type: ignore
        y = x + 1
        if y > 10:
            z = y
        else:
            z = y + 1.2
        return z

    result = TypeInfer(dialects=unstable.dialects, no_raise=False).fixpoint(unstable)
    assert (
        result.has_done_something
    )  # this will always be true because TypeInfer always rewrites type
