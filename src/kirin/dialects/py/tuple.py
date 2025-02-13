"""The tuple dialect for Python.

This dialect provides a way to work with Python tuples in the IR, including:

- The `New` statement class.
- The lowering pass for the tuple statement.
- The concrete implementation of the tuple statement.
- The type inference implementation of the tuple addition with `py.binop.Add`.
- The constant propagation implementation of the tuple statement.
- The Julia emitter for the tuple statement.

This dialect maps `ast.Tuple` nodes to the `New` statement.
"""

import ast

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.analysis import const
from kirin.emit.julia import EmitJulia, EmitStrFrame
from kirin.dialects.eltype import ElType
from kirin.dialects.py.binop import Add

dialect = ir.Dialect("py.tuple")


@statement(dialect=dialect)
class New(ir.Statement):
    traits = frozenset({ir.Pure(), ir.FromPythonCall()})
    result: ir.ResultValue = info.result()

    def __init__(self, values: tuple[ir.SSAValue, ...]) -> None:
        result_type = types.Generic(tuple, *tuple(value.type for value in values))
        super().__init__(
            args=values,
            result_types=[
                result_type,
            ],
        )


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(New)
    def new(self, interp: interp.Interpreter, frame: interp.Frame, stmt: New):
        return (frame.get_values(stmt.args),)


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(ElType, types.PyClass(tuple))
    def eltype_tuple(self, interp, frame: interp.Frame, stmt: ElType):
        tuple_type = frame.get(stmt.container)
        if isinstance(tuple_type, types.Generic):
            ret = tuple_type.vars[0]
            for var in tuple_type.vars[1:]:
                ret = ret.join(var)
            return (ret,)
        else:
            return (types.Any,)

    @interp.impl(Add, types.PyClass(tuple), types.PyClass(tuple))
    def add(self, interp, frame: interp.Frame[types.TypeAttribute], stmt):
        lhs = frame.get(stmt.lhs)
        rhs = frame.get(stmt.rhs)
        if isinstance(lhs, types.Generic) and isinstance(rhs, types.Generic):
            return (types.Generic(tuple, *(lhs.vars + rhs.vars)),)
        else:
            return (types.PyClass(tuple),)  # no type param, so unknown


@dialect.register(key="constprop")
class ConstPropTable(interp.MethodTable):

    @interp.impl(New)
    def new_tuple(
        self,
        _: const.Propagate,
        frame: const.Frame,
        stmt: New,
    ) -> interp.StatementResult[const.Result]:
        return (const.PartialTuple(tuple(x for x in frame.get_values(stmt.args))),)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Tuple(
        self, state: lowering.LoweringState, node: ast.Tuple
    ) -> lowering.Result:
        return lowering.Result(
            state.append_stmt(
                stmt=New(tuple(state.visit(elem).expect_one() for elem in node.elts))
            )
        )


@dialect.register(key="emit.julia")
class JuliaTable(interp.MethodTable):

    @interp.impl(New)
    def emit_NewTuple(self, emit: EmitJulia, frame: EmitStrFrame, stmt: New):
        return (
            emit.write_assign(
                frame, stmt.result, "(" + ", ".join(frame.get_values(stmt.args)) + ")"
            ),
        )
