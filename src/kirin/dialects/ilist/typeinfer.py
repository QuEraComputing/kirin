from kirin.ir import types
from kirin.interp import Frame, MethodTable, impl
from kirin.analysis import const
from kirin.dialects.py.binop import Add
from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.py.indexing import GetItem

from .stmts import Push, IListType
from .runtime import IList
from ._dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(MethodTable):

    @staticmethod
    def _get_list_len(typ: types.Generic):
        if isinstance(typ.vars[1], types.Literal) and isinstance(typ.vars[1].data, int):
            return typ.vars[1].data
        else:
            return types.Any

    @impl(Push)
    def push(
        self, interp: TypeInference, frame: Frame[types.TypeAttribute], stmt: Push
    ):
        lst_type: types.Generic = frame.get(stmt.lst)  # type: ignore
        value_type = frame.get(stmt.value)
        if not lst_type.is_subseteq(IListType):
            return (types.Bottom,)

        if not lst_type.vars[0].is_subseteq(value_type):
            return (types.Bottom,)

        lst_len = self._get_list_len(lst_type)
        if not isinstance(lst_len, int):
            return (IListType[lst_type.vars[0], types.Any],)

        return (IListType[lst_type.vars[0], types.Literal(lst_len + 1)],)

    @impl(Add, types.PyClass(IList), types.PyClass(IList))
    def add(self, interp: TypeInference, frame: Frame[types.TypeAttribute], stmt: Add):
        lhs_type = frame.get(stmt.lhs)
        rhs_type = frame.get(stmt.rhs)
        if not lhs_type.is_subseteq(IListType) or not rhs_type.is_subseteq(IListType):
            return (types.Bottom,)

        lhs_type = types.unwrap_hinted(lhs_type)
        rhs_type = types.unwrap_hinted(rhs_type)

        if not isinstance(lhs_type, types.Generic):  # just annotated with list
            lhs_type = IListType[types.Any, types.Any]

        if not isinstance(rhs_type, types.Generic):
            rhs_type = IListType[types.Any, types.Any]

        if len(lhs_type.vars) != 2 or len(rhs_type.vars) != 2:
            raise TypeError("missing type argument for list")

        elem_type = lhs_type.vars[0].join(rhs_type.vars[0])

        lhs_len = self._get_list_len(lhs_type)
        rhs_len = self._get_list_len(rhs_type)
        if isinstance(lhs_len, int) and isinstance(rhs_len, int):
            return (IListType[elem_type, types.Literal(lhs_len + rhs_len)],)
        return (IListType[elem_type, types.Any],)

    @impl(GetItem, types.PyClass(IList), types.PyClass(int))
    def getitem(
        self, interp: TypeInference, frame: Frame[types.TypeAttribute], stmt: GetItem
    ):
        obj_type = frame.get(stmt.obj)
        if not obj_type.is_subseteq(IListType):
            raise TypeError(f"Expected list, got {obj_type}")

        # just list type
        if not isinstance(obj_type, types.Generic):
            return (types.Any,)
        else:
            return (obj_type.vars[0],)

    @impl(GetItem, types.PyClass(IList), types.PyClass(slice))
    def getitem_slice(
        self, interp: TypeInference, frame: Frame[types.TypeAttribute], stmt: GetItem
    ):
        obj_type = frame.get(stmt.obj)
        if not obj_type.is_subseteq(IListType):
            raise TypeError(f"Expected list, got {obj_type}")

        # just list type
        index_type = frame.get(stmt.index)
        if not isinstance(obj_type, types.Generic):
            return (IListType[types.Any, types.Any],)
        elif isinstance(index_type, types.Hinted) and isinstance(
            index_type.data, const.Value
        ):
            # TODO: actually calculate the size
            return (IListType[obj_type.vars[0], types.Any],)
        else:
            return (IListType[obj_type.vars[0], types.Any],)
