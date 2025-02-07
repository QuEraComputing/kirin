from kirin import types, interp
from kirin.dialects.py.binop import Add
from kirin.dialects.py.indexing import GetItem

from ._dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):

    @interp.impl(Add, types.PyClass(list), types.PyClass(list))
    def add(self, interp, frame: interp.Frame, stmt: Add):
        lhs_type = frame.get(stmt.lhs)
        rhs_type = frame.get(stmt.rhs)
        lhs_type = types.unwrap_hinted(lhs_type)
        rhs_type = types.unwrap_hinted(rhs_type)
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
        obj = frame.get(stmt.obj)
        obj_type = types.unwrap_hinted(obj)
        if isinstance(obj_type, types.Generic):
            return (obj_type.vars[0],)
        else:
            return (types.Any,)

    @interp.impl(GetItem, types.PyClass(list), types.PyClass(slice))
    def getitem_list_slice(
        self, interp, frame: interp.Frame[types.TypeAttribute], stmt: GetItem
    ):
        obj = frame.get(stmt.obj)
        obj_type = types.unwrap_hinted(obj)
        if isinstance(obj_type, types.Generic):
            return (types.List[obj_type.vars[0]],)
        else:
            return (types.Any,)
