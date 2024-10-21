from kirin import ir
from kirin.decl import info, statement
from kirin.dialects.py import stmts, types
from kirin.dialects.py.rules import RewriteGetItem
from kirin.prelude import basic_no_opt
from kirin.rewrite import Walk

dummy = ir.Dialect("dummy")


class RegGetItemInterface(stmts.GetItemLike["RegGetItem"]):

    def get_object(self, stmt: "RegGetItem") -> ir.SSAValue:
        return stmt.reg

    def get_index(self, stmt: "RegGetItem") -> ir.SSAValue:
        return stmt.index

    def new(
        self, stmt_type: type["RegGetItem"], obj: ir.SSAValue, index: ir.SSAValue
    ) -> "RegGetItem":
        return RegGetItem(obj, index)


class Register:
    pass


@statement(dialect=dummy)
class New(ir.Statement):
    name = "new"
    traits = frozenset({ir.Pure()})
    result: ir.ResultValue = info.result(types.PyClass(Register))


@statement(dialect=dummy)
class RegGetItem(ir.Statement):
    name = "reg.get"
    traits = frozenset({ir.Pure(), RegGetItemInterface()})
    reg: ir.SSAValue = info.argument(types.PyClass(Register))
    index: ir.SSAValue = info.argument(types.Int)
    result: ir.ResultValue = info.result(types.Int)


@basic_no_opt.add(dummy)
def main():
    reg = New()
    return reg[0]  # type: ignore


def test_rewrite_getitem():
    rule = Walk(RewriteGetItem(RegGetItem, types.PyClass(Register)))

    stmt: ir.Statement = main.code.body.blocks[0].stmts.at(-2)  # type: ignore
    assert isinstance(stmt, stmts.GetItem)
    rule.rewrite(main.code)
    stmt: ir.Statement = main.code.body.blocks[0].stmts.at(-2)  # type: ignore
    assert isinstance(stmt, RegGetItem)
