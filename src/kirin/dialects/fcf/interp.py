from kirin import ir
from kirin.interp import Err, Frame, Interpreter, MethodTable, impl
from kirin.dialects.fcf.stmts import Map, Scan, Foldl, Foldr
from kirin.dialects.fcf.dialect import dialect


@dialect.register
class FCFInterpreter(MethodTable):

    @impl(Foldl)
    def foldl(self, interp: Interpreter, frame: Frame, stmt: Foldl):
        fn: ir.Method = frame.get(stmt.fn)
        coll = frame.get(stmt.coll)
        init = frame.get(stmt.init)

        acc = init
        for elem in coll:
            # NOTE: assume fn has been type checked
            acc = interp.eval(fn, (acc, elem)).value
            if isinstance(acc, Err):
                return acc
        return (acc,)

    @impl(Foldr)
    def foldr(self, interp: Interpreter, frame: Frame, stmt: Foldr):
        fn: ir.Method = frame.get(stmt.fn)
        coll = frame.get(stmt.coll)
        init = frame.get(stmt.init)

        acc = init
        for elem in reversed(coll):
            # NOTE: assume fn has been type checked
            acc = interp.eval(fn, (elem, acc)).value
            if isinstance(acc, Err):
                return acc
        return (acc,)

    @impl(Map)
    def map_tuple(self, interp: Interpreter, frame: Frame, stmt: Map):
        fn: ir.Method = frame.get(stmt.fn)
        coll = frame.get(stmt.coll)
        ret = []
        for elem in coll:
            # NOTE: assume fn has been type checked
            _ret = interp.eval(fn, (elem,)).value
            if isinstance(_ret, Err):
                return _ret
            else:
                ret.append(_ret)
        return (tuple(ret),)

    @impl(Scan)
    def scan(self, interp: Interpreter, frame: Frame, stmt: Scan):
        fn: ir.Method = frame.get(stmt.fn)
        init = frame.get(stmt.init)
        coll = frame.get(stmt.coll)

        carry = init
        ys = []
        for elem in coll:
            # NOTE: assume fn has been type checked
            _acc = interp.eval(fn, (carry, elem)).value
            if isinstance(_acc, Err):
                return _acc
            else:
                carry, y = _acc
                ys.append(y)
        return ((carry, ys),)
