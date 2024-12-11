from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.cf.dialect import dialect
from kirin.dialects.cf.stmts import Assert, Branch, ConditionalBranch
from kirin.interp import DialectInterpreter, ResultValue, Successor, impl
from kirin.ir import types


@dialect.register(key="typeinfer")
class TypeInfer(DialectInterpreter):

    @impl(Assert)
    def assert_stmt(self, interp: TypeInference, stmt: Assert, values):
        return ResultValue(types.Bottom)

    @impl(Branch)
    def branch(self, interp: TypeInference, stmt: Branch, values):
        frame = interp.state.current_frame()
        frame.worklist.append(Successor(stmt.successor, *values))
        return ResultValue()

    @impl(ConditionalBranch)
    def conditional_branch(
        self, interp: TypeInference, stmt: ConditionalBranch, values
    ):
        frame = interp.state.current_frame()
        frame.worklist.append(
            Successor(stmt.else_successor, *frame.get_values(stmt.else_arguments))
        )
        frame.worklist.append(
            Successor(stmt.then_successor, *frame.get_values(stmt.then_arguments))
        )
        return ResultValue()
