from kirin.ir import types
from kirin.interp import Frame, MethodTable, impl
from kirin.dialects.py.binop import Add
from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.py.indexing import GetItem

from ._dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(MethodTable):

    @impl(Add, types.PyClass(list), types.PyClass(list))
    def add(self, interp: TypeInference, frame: Frame, stmt: Add):
        if not stmt.lhs.type.is_subseteq(types.List) or not stmt.rhs.type.is_subseteq(
            types.List
        ):
            raise TypeError(f"Expected list, got {stmt.lhs.type}")

        if not isinstance(stmt.lhs.type, types.Generic):
            lhs_type = types.Generic(list, types.Any)
        else:
            lhs_type = stmt.lhs.type

        if not isinstance(stmt.rhs.type, types.Generic):
            rhs_type = types.Generic(list, types.Any)
        else:
            rhs_type = stmt.rhs.type

        if len(lhs_type.vars) != 1 or len(rhs_type.vars) != 1:
            raise TypeError("missing type argument for list")

        return (types.List[lhs_type.vars[0].join(rhs_type.vars[0])],)

    @impl(GetItem, types.PyClass(list), types.PyClass(int))
    def getitem(self, interp: TypeInference, frame: Frame, stmt: GetItem):
        if not stmt.obj.type.is_subseteq(types.List):
            raise TypeError(f"Expected list, got {stmt.obj.type}")

        if not isinstance(stmt.obj.type, types.Generic):
            return (types.Any,)
        else:
            return (stmt.obj.type.vars[0],)
