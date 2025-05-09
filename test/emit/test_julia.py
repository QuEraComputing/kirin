from pathlib import Path

from kirin import emit
from kirin.prelude import structural
from kirin.dialects import debug


@structural.add(debug)
def some_arith(x: int, y: float):
    return x + y


@structural.add(debug)
def julia_like(x: int, y: int):
    for i in range(x):
        for j in range(y):
            if i == 0:
                debug.info("Hello")
            else:
                debug.info("World")
    return some_arith(x + y, 4.0)


def test_julia_like(tmp_path):
    file = tmp_path / "julia_like.jl"
    with open(file, "w") as io:
        julia_emit = emit.Julia(structural.add(debug), io=io)
        julia_emit.run(julia_like)

    with open(file, "r") as io:
        generated = io.read()

    with open(Path(__file__).parent / "julia_like.jl", "r") as io:
        target = io.read()

    assert generated.strip() == target.strip()
