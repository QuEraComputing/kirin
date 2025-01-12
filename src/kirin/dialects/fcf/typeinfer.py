from typing import Callable, Iterable

from kirin import ir
from kirin.interp import Err, MethodTable, AbstractFrame, impl
from kirin.analysis.typeinfer import TypeInference
from kirin.dialects.fcf.stmts import Map, Scan, Foldl, Foldr
from kirin.dialects.fcf.dialect import dialect


@dialect.register(key="typeinfer")
class TypeInfer(MethodTable):

    @impl(Foldl)
    def foldl(
        self,
        interp: TypeInference,
        frame: AbstractFrame,
        stmt: Foldl,
    ):
        return self.fold(lambda x: x, interp, stmt, frame.get_values(stmt.args))

    @impl(Foldr)
    def foldr(
        self,
        interp: TypeInference,
        frame: AbstractFrame,
        stmt: Foldr,
    ):
        return self.fold(reversed, interp, stmt, frame.get_values(stmt.args))

    def fold(
        self,
        order: Callable[
            [tuple[ir.types.TypeAttribute, ...]], Iterable[ir.types.TypeAttribute]
        ],
        interp: TypeInference,
        stmt: Foldl | Foldr,
        values: tuple[ir.types.TypeAttribute, ...],
    ):
        if not interp.is_const(values[0]):
            return (stmt.result.type,)  # give up on dynamic calls

        fn: ir.Method = values[0].data.data
        coll: ir.types.TypeAttribute = values[1]
        init: ir.types.TypeAttribute = values[2]

        if isinstance(coll, ir.types.Generic):
            if coll.is_subseteq(ir.types.List):
                ret = interp.eval(fn, (init, coll.vars[0])).value
                if isinstance(ret, Err):
                    return ret

                if not init.is_subseteq(ret):
                    return (ir.types.Bottom,)
                return (ret,)
            elif coll.is_subseteq(ir.types.Tuple):
                carry = init
                for elem in order(coll.vars):
                    carry = interp.eval(fn, (carry, elem)).value
                    if isinstance(carry, Err):
                        return carry
                return (carry,)

        return (ir.types.Bottom,)

    @impl(Map, ir.types.PyClass(ir.Method), ir.types.PyClass(list))
    def map_list(
        self,
        interp: TypeInference,
        frame: AbstractFrame,
        stmt: Map,
    ):
        fn_value = frame.get(stmt.fn)
        if not interp.is_const(fn_value):
            return (ir.types.List[ir.types.Any],)  # give up on dynamic calls

        fn: ir.Method = fn_value.data.data
        coll: ir.types.TypeAttribute = frame.get(stmt.coll)
        if isinstance(coll, ir.types.Generic) and coll.is_subseteq(ir.types.List):
            elem = interp.eval(fn, (coll.vars[0],)).value
            if isinstance(elem, Err):
                # fn errors forward the error
                return elem
            return (ir.types.List[elem],)
        return (ir.types.Bottom,)

    @impl(Map, ir.types.PyClass(ir.Method), ir.types.PyClass(range))
    def map_range(
        self,
        interp: TypeInference,
        frame: AbstractFrame,
        stmt: Map,
    ):
        fn_value = frame.get(stmt.fn)
        if not interp.is_const(fn_value):
            return (ir.types.List,)  # give up on dynamic calls

        fn: ir.Method = fn_value.data.data
        elem = interp.eval(fn, (ir.types.Int,)).value
        # fn errors forward the error
        if isinstance(elem, Err):
            return elem
        else:
            return (ir.types.List[elem],)

    @impl(Scan)
    def scan(
        self,
        interp: TypeInference,
        frame: AbstractFrame,
        stmt: Scan,
    ):
        fn_value = frame.get(stmt.fn)
        init = frame.get(stmt.init)
        coll = frame.get(stmt.coll)

        if not interp.is_const(fn_value):
            return (ir.types.Tuple[init, ir.types.List[ir.types.Any]],)

        fn: ir.Method = fn_value.data.data
        if isinstance(coll, ir.types.Generic) and coll.is_subseteq(ir.types.List):
            ret = interp.eval(fn, (init, coll.vars[0])).value
            if isinstance(ret, Err):
                return ret

            if isinstance(ret, ir.types.Generic) and ret.is_subseteq(ir.types.Tuple):
                if len(ret.vars) != 2:
                    return (ir.types.Bottom,)
                carry: ir.types.TypeAttribute = ret.vars[0]
                if not carry.is_subseteq(init):
                    return (ir.types.Bottom,)
                return (ir.types.Tuple[carry, ir.types.List[ret.vars[1]]],)

        return (ir.types.Bottom,)
