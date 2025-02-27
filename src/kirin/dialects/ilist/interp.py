from kirin import ir, types
from kirin.interp import Frame, Interpreter, MethodTable, impl
from kirin.dialects.py.len import Len
from kirin.dialects.py.binop import Add

from .stmts import Map, New, Push, Scan, Foldl, Foldr, Range, ForEach
from .runtime import IList
from ._dialect import dialect


@dialect.register
class IListInterpreter(MethodTable):

    @impl(Range)
    def _range(self, interp, frame: Frame, stmt: Range):
        return (IList(range(*frame.get_values(stmt.args))),)

    @impl(New)
    def new(self, interp, frame: Frame, stmt: New):
        return (IList(list(frame.get_values(stmt.values))),)

    @impl(Len, types.PyClass(IList))
    def len(self, interp, frame: Frame, stmt: Len):
        return (len(frame.get(stmt.value).data),)

    @impl(Add, types.PyClass(IList), types.PyClass(IList))
    def add(self, interp, frame: Frame, stmt: Add):
        return (IList(frame.get(stmt.lhs).data + frame.get(stmt.rhs).data),)

    @impl(Push)
    def push(self, interp, frame: Frame, stmt: Push):
        return (IList(frame.get(stmt.lst).data + [frame.get(stmt.value)]),)

    @impl(Map)
    def map(self, interp: Interpreter, frame: Frame, stmt: Map):
        fn: ir.Method = frame.get(stmt.fn)
        coll: IList = frame.get(stmt.collection)
        ret = []
        for elem in coll.data:
            # NOTE: assume fn has been type checked
            _, item = interp.run_method(fn, (elem,))
            ret.append(item)
        return (IList(ret),)

    @impl(Scan)
    def scan(self, interp: Interpreter, frame: Frame, stmt: Scan):
        fn: ir.Method = frame.get(stmt.fn)
        init = frame.get(stmt.init)
        coll: IList = frame.get(stmt.collection)

        carry = init
        ys = []
        for elem in coll.data:
            # NOTE: assume fn has been type checked
            _, (carry, y) = interp.run_method(fn, (carry, elem))
            ys.append(y)
        return ((carry, IList(ys)),)

    @impl(Foldr)
    def foldr(self, interp: Interpreter, frame: Frame, stmt: Foldr):
        return self.fold(interp, frame, stmt, reversed(frame.get(stmt.collection).data))

    @impl(Foldl)
    def foldl(self, interp: Interpreter, frame: Frame, stmt: Foldl):
        return self.fold(interp, frame, stmt, frame.get(stmt.collection).data)

    def fold(self, interp: Interpreter, frame: Frame, stmt: Foldr | Foldl, coll):
        fn: ir.Method = frame.get(stmt.fn)
        init = frame.get(stmt.init)

        acc = init
        for elem in coll:
            # NOTE: assume fn has been type checked
            _, acc = interp.run_method(fn, (acc, elem))
        return (acc,)

    @impl(ForEach)
    def for_each(self, interp: Interpreter, frame: Frame, stmt: ForEach):
        fn: ir.Method = frame.get(stmt.fn)
        coll: IList = frame.get(stmt.collection)
        for elem in coll.data:
            # NOTE: assume fn has been type checked
            interp.run_method(fn, (elem,))
        return (None,)
