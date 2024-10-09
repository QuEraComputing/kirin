from kirin import ir
from kirin.dialects.fcf.dialect import dialect
from kirin.dialects.fcf.stmts import Foldl, Foldr, Map, Scan
from kirin.interp import DialectInterpreter, Interpreter, ResultValue, impl


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
            if isinstance(_acc, ResultValue):
                acc = _acc.values[0]
            else:
                return _acc
        return ResultValue(acc)

    @impl(Foldr)
    def foldr(self, interp: Interpreter, stmt: Foldr, values: tuple):
        fn: ir.Method = values[0]
        coll = values[1]
        init = values[2]

        acc = init
        for elem in reversed(coll):
            # NOTE: assume fn has been type checked
            _acc = interp.eval(fn, (elem, acc)).to_result()
            if isinstance(_acc, ResultValue):
                acc = _acc.values[0]
            else:
                return _acc
        return ResultValue(acc)

    @impl(Map)
    def map_list(self, interp: Interpreter, stmt: Map, values: tuple):
        fn: ir.Method = values[0]
        coll = values[1]
        ret = []
        for elem in coll:
            # NOTE: assume fn has been type checked
            _ret = interp.eval(fn, (elem,)).to_result()
            if isinstance(_ret, ResultValue):
                ret.append(_ret.values[0])
            else:
                return _ret
        return ResultValue(ret)

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
            if isinstance(_acc, ResultValue):
                carry, y = _acc.values[0]
                ys.append(y)
            else:  # fn err or no return
                return _acc
        return ResultValue((carry, ys))
