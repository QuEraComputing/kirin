from kirin.interp import FrameABC, Successor, MethodTable, impl
from kirin.analysis import const
from kirin.dialects.cf.stmts import Branch, ConditionalBranch
from kirin.dialects.cf.dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(MethodTable):

    @impl(Branch)
    def branch(
        self, interp: const.Propagate, frame: FrameABC[const.JointResult], stmt: Branch
    ):
        interp.state.current_frame().worklist.append(
            Successor(stmt.successor, *frame.get_values(stmt.arguments))
        )
        return ()

    @impl(ConditionalBranch)
    def conditional_branch(
        self,
        interp: const.Propagate,
        frame: FrameABC[const.JointResult],
        stmt: ConditionalBranch,
    ):
        frame = interp.state.current_frame()
        cond = frame.get(stmt.cond)
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
