from kirin.analysis.dataflow.constprop import (
    Const,
    ConstProp,
    ConstPropLattice,
    NotConst,
)
from kirin.dialects.cf.dialect import dialect
from kirin.dialects.cf.stmts import Assert, Branch, ConditionalBranch
from kirin.interp import DialectInterpreter, ResultValue, Successor, impl


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(Assert)
    def assert_stmt(self, interp: ConstProp, stmt: Assert, values):
        return ResultValue(NotConst())

    @impl(Branch)
    def branch(self, interp: ConstProp, stmt: Branch, values: tuple):
        print(values)
        interp.worklist.push(Successor(stmt.successor, *values))
        return ResultValue()

    @impl(ConditionalBranch)
    def conditional_branch(
        self,
        interp: ConstProp,
        stmt: ConditionalBranch,
        values: tuple[ConstPropLattice, ...],
    ):
        frame = interp.state.current_frame()
        cond = values[0]
        else_successor = Successor(
            stmt.else_successor, *frame.get_values(stmt.else_arguments)
        )
        then_successor = Successor(
            stmt.then_successor, *frame.get_values(stmt.then_arguments)
        )
        if isinstance(cond, Const):
            if cond.data:
                interp.worklist.push(then_successor)
            else:
                interp.worklist.push(else_successor)
        else:
            interp.worklist.push(else_successor)
            interp.worklist.push(then_successor)
        return ResultValue()
