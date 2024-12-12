from kirin.interp import Successor, DialectInterpreter, impl
from kirin.analysis import const
from kirin.dialects.cf.stmts import Assert, Branch, ConditionalBranch
from kirin.dialects.cf.dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(DialectInterpreter):

    @impl(Assert)
    def assert_stmt(self, interp: const.Propagate, stmt: Assert, values):
        return ()

    @impl(Branch)
    def branch(self, interp: const.Propagate, stmt: Branch, values: tuple):
        interp.state.current_frame().worklist.append(Successor(stmt.successor, *values))
        return ()

    @impl(ConditionalBranch)
    def conditional_branch(
        self,
        interp: const.Propagate,
        stmt: ConditionalBranch,
        values: tuple[const.JointResult, ...],
    ):
        frame = interp.state.current_frame()
        cond = values[0]
        if isinstance(cond.const, const.Value):
            else_successor = Successor(
                stmt.else_successor, *frame.get_values(stmt.else_arguments)
            )
            then_successor = Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
            if cond.const.data:
                frame.worklist.append(then_successor)
            else:
                frame.worklist.append(else_successor)
        else:
            frame.entries[stmt.cond] = const.JointResult(const.Value(True), cond.purity)
            then_successor = Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
            frame.worklist.append(then_successor)

            frame.entries[stmt.cond] = const.JointResult(
                const.Value(False), cond.purity
            )
            else_successor = Successor(
                stmt.else_successor, *frame.get_values(stmt.else_arguments)
            )
            frame.worklist.append(else_successor)

            frame.entries[stmt.cond] = cond
        return ()
