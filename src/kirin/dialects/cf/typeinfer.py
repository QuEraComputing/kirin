from kirin.ir import types
from kirin.interp import Successor, MethodTable, AbstractFrame, impl
from kirin.dialects.cf.stmts import Assert, Branch, ConditionalBranch
from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.cf.dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(MethodTable):

    @impl(Assert)
    def assert_stmt(self, interp: TypeInference, frame: AbstractFrame, stmt: Assert):
        return (types.Bottom,)

    @impl(Branch)
    def branch(self, interp: TypeInference, frame: AbstractFrame, stmt: Branch):
        frame.worklist.append(
            Successor(stmt.successor, *frame.get_values(stmt.arguments))
        )
        return ()

    @impl(ConditionalBranch)
    def conditional_branch(
        self, interp: TypeInference, frame: AbstractFrame, stmt: ConditionalBranch
    ):
        frame.worklist.append(
            Successor(stmt.else_successor, *frame.get_values(stmt.else_arguments))
        )
        frame.worklist.append(
            Successor(stmt.then_successor, *frame.get_values(stmt.then_arguments))
        )
        return ()
