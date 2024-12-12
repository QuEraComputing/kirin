from kirin.interp import Err, Successor, Interpreter, DialectInterpreter, impl
from kirin.dialects.cf.stmts import Assert, Branch, ConditionalBranch
from kirin.dialects.cf.dialect import dialect


@dialect.register
class CfInterpreter(DialectInterpreter):

    @impl(Assert)
    def assert_stmt(self, interp: Interpreter, stmt: Assert, values):
        if values[0] is True:
            return ()

        if stmt.message:
            return Err(AssertionError(values[1]), interp.state.frames)
        else:
            return Err(AssertionError(), interp.state.frames)

    @impl(Branch)
    def branch(self, ctx, stmt: Branch, values):
        return Successor(stmt.successor, *values)

    @impl(ConditionalBranch)
    def conditional_branch(self, interp: Interpreter, stmt: ConditionalBranch, values):
        frame = interp.state.current_frame()
        if values[0]:
            return Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
        else:
            return Successor(
                stmt.else_successor, *frame.get_values(stmt.else_arguments)
            )
