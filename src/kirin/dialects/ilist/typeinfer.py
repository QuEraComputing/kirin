from kirin import ir, types
from kirin.decl import fields
from kirin.interp import Frame, MethodTable, impl
from kirin.dialects.eltype import ElType
from kirin.dialects.py.binop import Add
from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.py.indexing import GetItem
from kirin.analysis.typeinfer.widen import widen

from .stmts import New, Push, Scan, Foldl, Foldr, Range, IListType
from .runtime import IList
from ._dialect import dialect

# Precision/performance heuristic: allow structural widening of the accumulator
# before falling back to top, mirroring scf.For loop type inference.
_FOLD_WIDENING_BUDGET = 16


@dialect.register(key="typeinfer")
class TypeInfer(MethodTable):

    @impl(Range)
    def range(
        self, interp_: TypeInference, frame: Frame[types.TypeAttribute], stmt: Range
    ):
        result = interp_.maybe_const(stmt.result, IList)
        if result:
            return (IListType[types.Int, types.Literal(len(result))],)
        return (IListType[types.Int, types.Any],)

    @staticmethod
    def _get_list_len(typ: types.Generic):
        if isinstance(typ.vars[1], types.Literal) and isinstance(typ.vars[1].data, int):
            return typ.vars[1].data
        else:
            return types.Any

    @impl(ElType, types.PyClass(IList))
    def eltype_list(
        self, interp: TypeInference, frame: Frame[types.TypeAttribute], stmt: ElType
    ):
        list_type = frame.get(stmt.container)
        if isinstance(list_type, types.Generic):
            return (list_type.vars[0],)
        else:
            return (types.Any,)

    @impl(New)
    def new(self, interp: TypeInference, frame: Frame[types.TypeAttribute], stmt: New):
        values = frame.get_values(stmt.values)
        if not values:
            return (IListType[stmt.elem_type, types.Literal(0)],)

        elem_type = values[0]
        for v in values:
            elem_type = elem_type.join(v)

        return (IListType[elem_type, types.Literal(len(values))],)

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
        if not isinstance(obj_type, types.Generic):
            return (IListType[types.Any, types.Any],)
        elif index_ := interp.maybe_const(stmt.index, slice):
            # TODO: actually calculate the size
            obj_len = obj_type.vars[1]
            if not isinstance(obj_len, types.Literal):
                return (IListType[obj_type.vars[0], types.Any],)
            assert isinstance(obj_len.data, int)
            LenT = types.Literal(len(range(obj_len.data)[index_]))
            return (IListType[obj_type.vars[0], LenT],)
        else:
            return (IListType[obj_type.vars[0], types.Any],)

    @impl(Foldl)
    @impl(Foldr)
    def fold(
        self,
        interp: TypeInference,
        frame: Frame[types.TypeAttribute],
        stmt: Foldl | Foldr,
    ):
        coll = frame.get(stmt.collection)
        elem = coll.vars[0] if isinstance(coll, types.Generic) else types.Any

        # The result is `init` after zero steps or the return of `fn` after one or
        # more.
        result = frame.get(stmt.init)
        for _ in range(_FOLD_WIDENING_BUDGET):
            fn_ret = self._call_fn(interp, frame, stmt.fn, result, elem)
            if fn_ret.is_subseteq(result):
                break
            result = widen(result, fn_ret)
        else:
            result = types.Any

        self._refine_operands(frame, stmt)
        return (result,)

    @impl(Scan)
    def scan(
        self, interp: TypeInference, frame: Frame[types.TypeAttribute], stmt: Scan
    ):
        coll = frame.get(stmt.collection)
        elem = coll.vars[0] if isinstance(coll, types.Generic) else types.Any

        # The carry is `init` after zero steps or the carry returned by `fn` after
        # one or more.
        carry = frame.get(stmt.init)
        for _ in range(_FOLD_WIDENING_BUDGET):
            fn_ret = self._call_fn(interp, frame, stmt.fn, carry, elem)
            if (
                isinstance(fn_ret, types.Generic)
                and fn_ret.is_subseteq(types.Tuple)
                and len(fn_ret.vars) == 2
            ):
                next_carry, out = fn_ret.vars
            else:
                next_carry, out = types.Any, types.Any
            if next_carry.is_subseteq(carry):
                break  # `out` is the output type under the final carry
            carry = widen(carry, next_carry)
        else:
            carry, out = types.Any, types.Any

        if isinstance(coll, types.Generic) and len(coll.vars) == 2:
            length = coll.vars[1]
        else:
            length = types.Any
        self._refine_operands(frame, stmt)
        return (types.Tuple[carry, IListType[out, length]],)

    @staticmethod
    def _call_fn(
        interp: TypeInference,
        frame: Frame[types.TypeAttribute],
        fn: ir.SSAValue,
        *args: types.TypeAttribute,
    ) -> types.TypeAttribute:
        """Return type of calling `fn` with `args`, or `Any` if it is unknown."""
        mt = interp.maybe_const(fn, ir.Method)
        if mt is not None and not mt.inferred:
            _, ret = interp.call(mt.code, interp.method_self(mt), *args)
        elif mt is not None:  # so we don't end up in infinite loop
            ret = mt.return_type
        elif isinstance(fn_type := frame.get(fn), types.FunctionType):
            ret = fn_type.return_type
        else:
            ret = None

        # Bottom means the analysis gave up (e.g. recursion limit), not that `fn`
        # never returns, and would make a fold collapse onto the type of `init`.
        if ret is None or ret.is_subseteq(types.Bottom):
            return types.Any
        return ret

    @staticmethod
    def _refine_operands(
        frame: Frame[types.TypeAttribute], stmt: Foldl | Foldr | Scan
    ) -> None:
        # Like `TypeInference.eval_fallback`, which types statements that have no
        # rule, refine operands to the declared types (e.g. an unannotated
        # collection becomes an IList) so the IR still type checks.
        for name, f in fields(stmt).args.items():
            arg = getattr(stmt, name)
            frame.set(arg, frame.get(arg).meet(f.type))
