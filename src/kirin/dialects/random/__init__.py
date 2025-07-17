from kirin import lowering

from . import stmts as stmts, interp as interp
from ._dialect import dialect as dialect


@lowering.wraps(stmts.Random)
def random() -> float:
    """
    Generate a random floating number between 0 and 1.
    """
    ...


@lowering.wraps(stmts.RandInt)
def randint(start: int, stop: int) -> int:
    """
    Generate a random integer between the given range.
    """
    ...


@lowering.wraps(stmts.Uniform)
def uniform(start: float, stop: float) -> float:
    """
    Generate a random floating number between the given range.
    """
    ...


@lowering.wraps(stmts.Seed)
def seed(value: int) -> None:
    """
    Set the seed for the random number generator.
    """
    ...
