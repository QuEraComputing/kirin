from kirin.analysis.dataflow.constprop import Const, ConstProp, ConstPropLattice
from kirin.dialects.cf.dialect import dialect
from kirin.dialects.cf.stmts import Assert, Branch, ConditionalBranch
from kirin.interp import DialectInterpreter, ResultValue, Successor, impl


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(Assert)
    def assert_stmt(self, interp: ConstProp, stmt: Assert, values):
        return ResultValue()

    @impl(Branch)
    def branch(self, interp: ConstProp, stmt: Branch, values: tuple):
        interp.state.current_frame().worklist.push(Successor(stmt.successor, *values))
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
        if isinstance(cond, Const):
            else_successor = Successor(
                stmt.else_successor, *frame.get_values(stmt.else_arguments)
            )
            then_successor = Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
            if cond.data:
                frame.worklist.push(then_successor)
            else:
                frame.worklist.push(else_successor)
        else:
            frame.entries[stmt.cond] = Const(True)
            then_successor = Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
            frame.worklist.push(then_successor)

            frame.entries[stmt.cond] = Const(False)
            else_successor = Successor(
                stmt.else_successor, *frame.get_values(stmt.else_arguments)
            )
            frame.worklist.push(else_successor)

            frame.entries[stmt.cond] = cond
        return ResultValue()
