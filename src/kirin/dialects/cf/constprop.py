from kirin.interp import Successor, MethodTable, impl
from kirin.analysis import const
from kirin.dialects.cf.stmts import Branch, ConditionalBranch
from kirin.dialects.cf.dialect import dialect


@dialect.register(key="constprop")
class DialectConstProp(MethodTable):

    @impl(Branch)
    def branch(self, interp: const.Propagate, frame: const.Frame, stmt: Branch):
        interp.state.current_frame().worklist.append(
            Successor(stmt.successor, *frame.get_values(stmt.arguments))
        )
        return ()

    @impl(ConditionalBranch)
    def conditional_branch(
        self,
        interp: const.Propagate,
        frame: const.Frame,
        stmt: ConditionalBranch,
    ):
        frame = interp.state.current_frame()
        cond = frame.get(stmt.cond)
        if isinstance(cond, const.Value):
            else_successor = Successor(
                stmt.else_successor, *frame.get_values(stmt.else_arguments)
            )
            then_successor = Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
            if cond.data:
                frame.worklist.append(then_successor)
            else:
                frame.worklist.append(else_successor)
        else:
            frame.entries[stmt.cond] = const.Value(True)
            then_successor = Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
            frame.worklist.append(then_successor)

            frame.entries[stmt.cond] = const.Value(False)
            else_successor = Successor(
                stmt.else_successor, *frame.get_values(stmt.else_arguments)
            )
            frame.worklist.append(else_successor)

            frame.entries[stmt.cond] = cond
        return ()
