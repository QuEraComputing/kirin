from kirin import ir, types
from kirin.rewrite.abc import RewriteRule
from kirin.rewrite.result import RewriteResult
from kirin.dialects.py.tuple import New as TupleNew
from kirin.dialects.func.stmts import Call
from kirin.dialects.ilist.stmts import Map, New, Scan, Foldl, Foldr, ForEach, IListType
from kirin.dialects.py.constant import Constant
from kirin.dialects.py.indexing import GetItem


class Unroll(RewriteRule):

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        return getattr(
            self, f"rewrite_{node.__class__.__name__}", self.rewrite_fallback
        )(node)

    def rewrite_fallback(self, node: ir.Statement) -> RewriteResult:
        return RewriteResult()

    def _get_collection_len(self, collection: ir.SSAValue):
        coll_type = collection.type
        if not isinstance(coll_type, types.Generic):
            return None

        if not coll_type.is_subseteq(IListType):
            return None

        if not (
            isinstance(coll_type.vars[1], types.Literal)
            and isinstance(coll_type.vars[1].data, int)
        ):
            return None

        return coll_type.vars[1].data

    def rewrite_Map(self, node: Map) -> RewriteResult:
        # NOTE: if node.collection is a constant, we can
        # just leave it because Map is pure, and this will
        # be folded.
        if (coll_len := self._get_collection_len(node.collection)) is None:
            return RewriteResult()

        new_elems: list[ir.SSAValue] = []
        for elt_idx in range(coll_len):
            index = Constant(elt_idx)
            index.insert_before(node)
            elt = GetItem(node.collection, index.result)
            elt.insert_before(node)
            fn_call = Call(node.fn, (elt.result,))
            fn_call.insert_before(node)
            new_elems.append(fn_call.result)

        node.replace_by(New(values=tuple(new_elems)))
        return RewriteResult(has_done_something=True)

    def rewrite_Scan(self, node: Scan) -> RewriteResult:
        if (coll_len := self._get_collection_len(node.collection)) is None:
            return RewriteResult()

        index_0 = Constant(0)
        index_1 = Constant(1)
        # index_0.result.name = "idx0"
        # index_1.result.name = "idx1"
        index_0.insert_before(node)
        index_1.insert_before(node)
        carry = node.init
        ys: list[ir.SSAValue] = []
        for elem_idx in range(coll_len):
            index = Constant(elem_idx)
            # index.result.name = f"idx_{elem_idx}"
            elt = GetItem(node.collection, index.result)
            fn_call = Call(node.fn, (carry, elt.result))
            carry_stmt = GetItem(fn_call.result, index_0.result)
            y_stmt = GetItem(fn_call.result, index_1.result)
            carry = carry_stmt.result
            ys.append(y_stmt.result)

            index.insert_before(node)
            elt.insert_before(node)
            fn_call.insert_before(node)
            carry_stmt.insert_before(node)
            y_stmt.insert_before(node)

        ys_stmt = New(values=tuple(ys))
        ys_stmt.insert_before(node)
        ret = TupleNew(values=(carry, ys_stmt.result))
        node.replace_by(ret)
        return RewriteResult(has_done_something=True)

    def rewrite_Foldr(self, node: Foldr) -> RewriteResult:
        return self._rewrite_fold(node, True)

    def rewrite_Foldl(self, node: Foldl) -> RewriteResult:
        return self._rewrite_fold(node, False)

    def _rewrite_fold(self, node: Foldr | Foldl, reversed: bool) -> RewriteResult:
        if (coll_len := self._get_collection_len(node.collection)) is None:
            return RewriteResult()

        acc = node.init
        for elem_idx in range(coll_len):
            if reversed:
                elem_idx = coll_len - elem_idx - 1
            index = Constant(elem_idx)
            index.insert_before(node)
            elt = GetItem(node.collection, index.result)
            elt.insert_before(node)

            acc_stmt = Call(node.fn, (acc, elt.result))
            acc_stmt.insert_before(node)
            acc = acc_stmt.result

        node.result.replace_by(acc)
        node.delete()
        return RewriteResult(has_done_something=True)

    def rewrite_ForEach(self, node: ForEach) -> RewriteResult:
        if (coll_len := self._get_collection_len(node.collection)) is None:
            return RewriteResult()

        for elem_idx in range(coll_len):
            index = Constant(elem_idx)
            index.insert_before(node)
            elt = GetItem(node.collection, index.result)
            elt.insert_before(node)
            fn_call = Call(node.fn, (elt.result,))
            fn_call.insert_before(node)

        node.delete()
        return RewriteResult(has_done_something=True)
