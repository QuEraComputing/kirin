from kirin import ir
from kirin.interp import Interpreter, DialectInterpreter, impl
from kirin.dialects.fcf.stmts import Map, Scan, Foldl, Foldr
from kirin.dialects.fcf.dialect import dialect


@dialect.register
class FCFInterpreter(DialectInterpreter):

    @impl(Foldl)
    def foldl(self, interp: Interpreter, stmt: Foldl, values: tuple):
        fn: ir.Method = values[0]
        coll = values[1]
        init = values[2]

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
    def foldr(self, interp: Interpreter, stmt: Foldr, values: tuple):
        fn: ir.Method = values[0]
        coll = values[1]
        init = values[2]

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
    def map_list(self, interp: Interpreter, stmt: Map, values: tuple):
        fn: ir.Method = values[0]
        coll = values[1]
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
    def scan(self, interp: Interpreter, stmt: Scan, values: tuple):
        fn: ir.Method = values[0]
        init = values[1]
        coll = values[2]

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
