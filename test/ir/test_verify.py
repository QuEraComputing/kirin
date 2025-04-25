import pytest

from kirin import ir, lowering
from kirin.decl import statement
from kirin.prelude import basic_no_opt

dialect = ir.Dialect("foo")


@statement(dialect=dialect)
class InvalidStmt(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})

    def check(self):
        raise ValueError("Never triggers")


@statement(dialect=dialect)
class InvalidType(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})

    def check_type(self):
        raise ValueError("Never triggers")


@ir.dialect_group(basic_no_opt.add(dialect))
def foo(self):
    def run_pass(mt):
        pass

    return run_pass


def test_invalid_stmt():
    @foo
    def test():
        InvalidStmt()

    with pytest.raises(Exception):
        test.verify()


def test_invalid_type():
    @foo
    def test():
        InvalidType()

    with pytest.raises(Exception):
        test.verify_type()
