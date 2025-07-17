import random

from kirin.interp import Frame, MethodTable, impl

from . import stmts
from ._dialect import dialect


@dialect.register
class RandomMethodTable(MethodTable):

    @impl(stmts.Random)
    def random(self, interp, frame: Frame, stmt: stmts.Random):
        return (random.random(),)

    @impl(stmts.RandInt)
    def randint(self, interp, frame: Frame, stmt: stmts.RandInt):
        start = frame.get(stmt.start)
        stop = frame.get(stmt.stop)
        return (random.randint(start, stop),)

    @impl(stmts.Uniform)
    def uniform(self, interp, frame: Frame, stmt: stmts.Uniform):
        start = frame.get(stmt.start)
        stop = frame.get(stmt.stop)
        return (random.uniform(start, stop),)

    @impl(stmts.Seed)
    def seed(self, interp, frame: Frame, stmt: stmts.Seed):
        seed_value = frame.get(stmt.value)
        random.seed(seed_value)
        return tuple()
