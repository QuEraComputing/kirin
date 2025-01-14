from kirin import ir
from kirin.interp import Err, Frame, Interpreter, MethodTable, impl
from kirin.dialects.py.len import Len
from kirin.dialects.py.binop import Add
from kirin.dialects.py.indexing import GetItem

from .stmts import Map, New, Scan, FoldL, FoldR, ForEach
from ._dialect import dialect


@dialect.register
class IListInterpreter(MethodTable):

    @impl(New)
    def new(self, interp, frame: Frame, stmt: New):
        return (list(frame.get_values(stmt.values)),)

    @impl(Len)
    def len(self, interp, frame: Frame, stmt: Len):
        return (len(frame.get(stmt.value)),)

    @impl(Add)
    def add(self, interp, frame: Frame, stmt: Add):
        return (frame.get(stmt.lhs) + frame.get(stmt.rhs),)

    @impl(GetItem)
    def get_item(self, interp, frame: Frame, stmt: GetItem):
        return (frame.get(stmt.obj)[frame.get(stmt.index)],)

    @impl(Map)
    def map(self, interp: Interpreter, frame: Frame, stmt: Map):
        fn: ir.Method = frame.get(stmt.fn)
        coll = frame.get(stmt.collection)
        ret = []
        for elem in coll:
            # NOTE: assume fn has been type checked
            _ret = interp.eval(fn, (elem,)).value
            if isinstance(_ret, Err):
                return _ret
            else:
                ret.append(_ret)
        return (ret,)

    @impl(Scan)
    def scan(self, interp: Interpreter, frame: Frame, stmt: Scan):
        fn: ir.Method = frame.get(stmt.fn)
        init = frame.get(stmt.init)
        coll = frame.get(stmt.collection)

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

    @impl(FoldR)
    def foldr(self, interp: Interpreter, frame: Frame, stmt: FoldR):
        return self.fold(interp, frame, stmt, reversed(frame.get(stmt.collection)))

    @impl(FoldL)
    def foldl(self, interp: Interpreter, frame: Frame, stmt: FoldL):
        return self.fold(interp, frame, stmt, frame.get(stmt.collection))

    def fold(self, interp: Interpreter, frame: Frame, stmt: FoldR | FoldL, coll):
        fn: ir.Method = frame.get(stmt.fn)
        init = frame.get(stmt.init)

        acc = init
        for elem in coll:
            # NOTE: assume fn has been type checked
            acc = interp.eval(fn, (acc, elem)).value
            if isinstance(acc, Err):
                return acc
        return (acc,)

    @impl(ForEach)
    def for_each(self, interp: Interpreter, frame: Frame, stmt: ForEach):
        fn: ir.Method = frame.get(stmt.fn)
        coll = frame.get(stmt.collection)
        for elem in coll:
            # NOTE: assume fn has been type checked
            _ret = interp.eval(fn, (elem,)).value
            if isinstance(_ret, Err):
                return _ret
        return (None,)
