from typing import Callable, Iterable

from kirin import ir
from kirin.analysis.dataflow.typeinfer import TypeInference
from kirin.dialects.fcf.dialect import dialect
from kirin.dialects.fcf.stmts import Foldl, Foldr, MapList, Scan
from kirin.dialects.py import types
from kirin.interp import DialectInterpreter, ResultValue, impl


@dialect.register(key="typeinfer")
class TypeInfer(DialectInterpreter):

    @impl(Foldl)
    def foldl(
        self, interp: TypeInference, stmt: Foldl, values: tuple[types.PyType, ...]
    ):
        return self.fold(lambda x: x, interp, stmt, values)

    @impl(Foldr)
    def foldr(
        self, interp: TypeInference, stmt: Foldr, values: tuple[types.PyType, ...]
    ):
        return self.fold(reversed, interp, stmt, values)

    def fold(
        self,
        order: Callable[[tuple[types.PyType, ...]], Iterable[types.PyType]],
        interp,
        stmt,
        values: tuple[types.PyType, ...],
    ):
        if not isinstance(values[0], types.PyConst):
            return ResultValue(stmt.result.type)  # give up on dynamic calls

        fn: ir.Method = values[0].data
        coll: types.PyType = values[1]
        init: types.PyType = values[2]

        if isinstance(coll, types.PyGeneric):
            if coll.is_subseteq(types.List):
                ret = interp.eval(fn, (init, coll.vars[0])).to_result()
                if isinstance(ret, ResultValue):
                    ret_type: types.PyType = ret.values[0]
                    if not init.is_subseteq(ret_type):
                        return ResultValue(types.Bottom)
                    return ResultValue(ret_type)
            elif coll.is_subseteq(types.Tuple):
                carry = init
                for elem in order(coll.vars):
                    carry = interp.eval(fn, (carry, elem)).to_result()
                    if isinstance(carry, ResultValue):
                        carry = carry.values[0]
                    else:
                        return carry
                return ResultValue(carry)

        return ResultValue(types.Bottom)

    @impl(MapList)
    def map_list(self, interp: TypeInference, stmt: MapList, values):
        if not isinstance(values[0], types.PyConst):
            return ResultValue(types.List)  # give up on dynamic calls

        fn: ir.Method = values[0].data
        coll: types.PyType = values[1]
        if isinstance(coll, types.PyGeneric) and coll.is_subseteq(types.List):
            elem = interp.eval(fn, (coll.vars[0],)).to_result()
            if isinstance(elem, ResultValue):
                return ResultValue(types.List[elem.values[0]])
            else:  # fn errors forward the error
                return elem
        return ResultValue(types.Bottom)

    @impl(Scan)
    def scan(self, interp: TypeInference, stmt: Scan, values: tuple[types.PyType, ...]):
        fn = values[0]
        init = values[1]
        coll = values[2]

        if not isinstance(fn, types.PyConst):
            return ResultValue(types.Tuple[init, types.List])

        fn: ir.Method = fn.data
        if isinstance(coll, types.PyGeneric) and coll.is_subseteq(types.List):
            _ret = interp.eval(fn, (init, coll.vars[0])).to_result()
            if isinstance(_ret, ResultValue) and len(_ret.values) == 1:
                ret = _ret.values[0]
                if isinstance(ret, types.PyGeneric) and ret.is_subseteq(types.Tuple):
                    if len(ret.vars) != 2:
                        return ResultValue(types.Bottom)
                    carry: types.PyType = ret.vars[0]
                    if not carry.is_subseteq(init):
                        return ResultValue(types.Bottom)
                    return ResultValue(types.Tuple[carry, types.List[ret.vars[1]]])
            else:  # fn errors forward the error
                return _ret

        return ResultValue(types.Bottom)
