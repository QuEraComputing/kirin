from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.prelude import basic_no_opt
from kirin.analysis import TypeInference

dialect = ir.Dialect("ssacfg_dependencies")
T = types.TypeVar("T")


@statement(dialect=dialect)
class TypeIdentity(ir.Statement):
    name = "identity"
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    value: ir.SSAValue = info.argument(T)
    result: ir.ResultValue = info.result(T)


@dialect.register(key="typeinfer")
class TypeIdentityInference(interp.MethodTable):
    @interp.impl(TypeIdentity)
    def identity(self, interp_, frame, stmt: TypeIdentity):
        return (frame.get(stmt.value),)


group = basic_no_opt.add(dialect)


@group
def loop_carried_type(condition: bool):
    value = 0
    observed = value
    for _ in range(2):
        if condition:
            observed = TypeIdentity(value)
            value = 1.0
    return observed


def test_typeinfer_revisits_dominated_loop_carried_use():
    method = loop_carried_type.similar()
    identity = next(stmt for stmt in method.code.walk() if stmt.name == "identity")

    frame, _ = TypeInference(method.dialects).run(method)

    assert frame.entries[identity.result] == types.Int.join(types.Float)
