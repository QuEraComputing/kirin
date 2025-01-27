from kirin.interp import Frame, Successor, Interpreter, MethodTable, WrapException, impl
from kirin.dialects.cf.stmts import Assert, Branch, ConditionalBranch
from kirin.dialects.cf.dialect import dialect


@dialect.register
class CfInterpreter(MethodTable):

    @impl(Assert)
    def assert_stmt(self, interp: Interpreter, frame: Frame, stmt: Assert):
        if frame.get(stmt.condition) is True:
            return ()

        if stmt.message:
            raise WrapException(AssertionError(frame.get(stmt.message)))
        else:
            raise WrapException(AssertionError("Assertion failed"))

    @impl(Branch)
    def branch(self, interp: Interpreter, frame: Frame, stmt: Branch):
        return Successor(stmt.successor, *frame.get_values(stmt.arguments))

    @impl(ConditionalBranch)
    def conditional_branch(
        self, interp: Interpreter, frame: Frame, stmt: ConditionalBranch
    ):
        if frame.get(stmt.cond):
            return Successor(
                stmt.then_successor, *frame.get_values(stmt.then_arguments)
            )
        else:
            return Successor(
                stmt.else_successor, *frame.get_values(stmt.else_arguments)
            )
