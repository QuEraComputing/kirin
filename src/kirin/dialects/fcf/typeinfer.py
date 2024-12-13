from typing import Callable, Iterable

from kirin import ir
from kirin.interp import MethodTable, impl
from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.fcf.stmts import Map, Scan, Foldl, Foldr
from kirin.dialects.fcf.dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(MethodTable):

    @impl(Foldl)
    def foldl(
        self,
        interp: TypeInference,
        stmt: Foldl,
        values: tuple[ir.types.TypeAttribute, ...],
    ):
        return self.fold(lambda x: x, interp, stmt, values)

    @impl(Foldr)
    def foldr(
        self,
        interp: TypeInference,
        stmt: Foldr,
        values: tuple[ir.types.TypeAttribute, ...],
    ):
        return self.fold(reversed, interp, stmt, values)

    def fold(
        self,
        order: Callable[
            [tuple[ir.types.TypeAttribute, ...]], Iterable[ir.types.TypeAttribute]
        ],
        interp: TypeInference,
        stmt,
        values: tuple[ir.types.TypeAttribute, ...],
    ):
        if not isinstance(values[0], ir.types.Const):
            return (stmt.result.type,)  # give up on dynamic calls

        fn: ir.Method = values[0].data
        coll: ir.types.TypeAttribute = values[1]
        init: ir.types.TypeAttribute = values[2]

        if isinstance(coll, ir.types.Generic):
            if coll.is_subseteq(ir.types.List):
                ret = interp.eval(fn, (init, coll.vars[0])).to_result()
                if isinstance(ret, tuple):
                    ret_type = ret[0]
                    if not init.is_subseteq(ret_type):
                        return (ir.types.Bottom,)
                    return (ret_type,)
            elif coll.is_subseteq(ir.types.Tuple):
                carry = init
                for elem in order(coll.vars):
                    carry = interp.eval(fn, (carry, elem)).to_result()
                    if isinstance(carry, tuple):
                        carry = carry[0]
                    else:
                        return carry
                return (carry,)

        return (ir.types.Bottom,)

    @impl(Map, ir.types.PyClass(ir.Method), ir.types.PyClass(list))
    def map_list(
        self,
        interp: TypeInference,
        stmt,
        values: tuple[ir.types.TypeAttribute, ir.types.TypeAttribute],
    ):
        if not isinstance(values[0], ir.types.Const):
            return (ir.types.List,)  # give up on dynamic calls

        fn: ir.Method = values[0].data
        coll: ir.types.TypeAttribute = values[1]
        if isinstance(coll, ir.types.Generic) and coll.is_subseteq(ir.types.List):
            elem = interp.eval(fn, (coll.vars[0],)).to_result()
            if isinstance(elem, tuple):
                return (ir.types.List[elem[0]],)
            else:  # fn errors forward the error
                return elem
        return (ir.types.Bottom,)

    @impl(Map, ir.types.PyClass(ir.Method), ir.types.PyClass(range))
    def map_range(
        self,
        interp: TypeInference,
        stmt,
        values: tuple[ir.types.TypeAttribute, ir.types.TypeAttribute],
    ):
        if not isinstance(values[0], ir.types.Const):
            return (ir.types.List,)  # give up on dynamic calls

        fn: ir.Method = values[0].data
        elem = interp.eval(fn, (ir.types.Int,)).to_result()
        if isinstance(elem, tuple):
            return (ir.types.List[elem[0]],)
        else:  # fn errors forward the error
            return elem

    @impl(Scan)
    def scan(
        self,
        interp: TypeInference,
        stmt: Scan,
        values: tuple[ir.types.TypeAttribute, ...],
    ):
        init = values[1]
        coll = values[2]

        if not isinstance(values[0], ir.types.Const):
            return (ir.types.Tuple[init, ir.types.List],)

        fn: ir.Method = values[0].data
        if isinstance(coll, ir.types.Generic) and coll.is_subseteq(ir.types.List):
            _ret = interp.eval(fn, (init, coll.vars[0])).to_result()
            if isinstance(_ret, tuple) and len(_ret) == 1:
                ret = _ret[0]
                if isinstance(ret, ir.types.Generic) and ret.is_subseteq(
                    ir.types.Tuple
                ):
                    if len(ret.vars) != 2:
                        return (ir.types.Bottom,)
                    carry: ir.types.TypeAttribute = ret.vars[0]
                    if not carry.is_subseteq(init):
                        return (ir.types.Bottom,)
                    return (ir.types.Tuple[carry, ir.types.List[ret.vars[1]]],)
            else:  # fn errors forward the error
                return _ret

        return (ir.types.Bottom,)
