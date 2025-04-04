from typing import Any, Generator
from contextlib import contextmanager

from kirin import ir, lowering
from kirin.decl import info, statement
from kirin.prelude import structural_no_opt
from kirin.dialects import ilist

dialect = ir.Dialect("with_binding")


@statement(dialect=dialect)
class ContextStatatement(ir.Statement):
    traits = frozenset({lowering.FromPythonWithSingleItem()})
    body: ir.Region = info.region(multi=False)


@ir.dialect_group(structural_no_opt.add(dialect))
def dummy(self):

    def run_pass(mt):

        return mt

    return run_pass


@lowering.wraps(ContextStatatement)
@contextmanager
def context_statement() -> Generator[Any, None, None]: ...


@dummy
def with_binding():
    x = 1

    def fn(x):
        return x**2

    with context_statement():
        with context_statement():
            x = ilist.map(fn, ilist.range(10))

    return x


def test_with_binding():
    stmt = with_binding.callable_region.blocks[0].stmts.at(-2)
    assert isinstance(stmt, ContextStatatement)
    assert len(stmt.body.blocks) == 1
    stmt = stmt.body.blocks[0].stmts.at(0)
    assert isinstance(stmt, ContextStatatement)
    assert len(stmt.body.blocks[0].stmts) == 5
