from kirin import types, interp
from kirin.dialects.eltype import ElType
from kirin.dialects.py.binop import Add
from kirin.dialects.py.indexing import GetItem

from ._dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(ElType, types.PyClass(list))
    def eltype_list(self, interp, frame: interp.Frame, stmt: ElType):
        list_type = frame.get(stmt.container)
        if isinstance(list_type, types.Generic):
            return (list_type.vars[0],)
        else:
            return (types.Any,)

    @interp.impl(Add, types.PyClass(list), types.PyClass(list))
    def add(self, interp, frame: interp.Frame, stmt: Add):
        lhs_type = frame.get(stmt.lhs)
        rhs_type = frame.get(stmt.rhs)
        if isinstance(lhs_type, types.Generic):
            lhs_elem_type = lhs_type.vars[0]
        else:
            lhs_elem_type = types.Any

        if isinstance(rhs_type, types.Generic):
            rhs_elem_type = rhs_type.vars[0]
        else:
            rhs_elem_type = types.Any

        return (types.List[lhs_elem_type.join(rhs_elem_type)],)

    @interp.impl(GetItem, types.PyClass(list), types.Int)
    def getitem_list_int(
        self, interp, frame: interp.Frame[types.TypeAttribute], stmt: GetItem
    ):
        obj_type = frame.get(stmt.obj)
        if isinstance(obj_type, types.Generic):
            return (obj_type.vars[0],)
        else:
            return (types.Any,)

    @interp.impl(GetItem, types.PyClass(list), types.PyClass(slice))
    def getitem_list_slice(
        self, interp, frame: interp.Frame[types.TypeAttribute], stmt: GetItem
    ):
        obj_type = frame.get(stmt.obj)
        if isinstance(obj_type, types.Generic):
            return (types.List[obj_type.vars[0]],)
        else:
            return (types.Any,)
