from kirin import ir
from kirin.interp import Frame, Interpreter, MethodTable, impl
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
            _acc = interp.eval(fn, (acc, elem)).to_result()
            if isinstance(_acc, tuple):
                acc = _acc[0]
            else:
                return _acc
        return (acc,)

    @impl(Foldr)
    def foldr(self, interp: Interpreter, frame: Frame, stmt: Foldr):
        fn: ir.Method = frame.get(stmt.fn)
        coll = frame.get(stmt.coll)
        init = frame.get(stmt.init)

        acc = init
        for elem in reversed(coll):
            # NOTE: assume fn has been type checked
            _acc = interp.eval(fn, (elem, acc)).to_result()
            if isinstance(_acc, tuple):
                acc = _acc[0]
            else:
                return _acc
        return (acc,)

    @impl(Map)
    def map_list(self, interp: Interpreter, frame: Frame, stmt: Map):
        fn: ir.Method = frame.get(stmt.fn)
        coll = frame.get(stmt.coll)
        ret = []
        for elem in coll:
            # NOTE: assume fn has been type checked
            _ret = interp.eval(fn, (elem,)).to_result()
            if isinstance(_ret, tuple):
                ret.append(_ret[0])
            else:
                return _ret
        return (ret,)

    @impl(Scan)
    def scan(self, interp: Interpreter, frame: Frame, stmt: Scan):
        fn: ir.Method = frame.get(stmt.fn)
        init = frame.get(stmt.init)
        coll = frame.get(stmt.coll)

        carry = init
        ys = []
        for elem in coll:
            # NOTE: assume fn has been type checked
            _acc = interp.eval(fn, (carry, elem)).to_result()
            if isinstance(_acc, tuple):
                carry, y = _acc[0]
                ys.append(y)
            else:  # fn err or no return
                return _acc
        return ((carry, ys),)
