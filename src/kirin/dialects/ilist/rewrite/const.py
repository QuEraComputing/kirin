from kirin import ir, types
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule
from kirin.rewrite.result import RewriteResult
from kirin.dialects.py.constant import Constant

from ..stmts import IListType
from ..runtime import IList


class ConstList2IList(RewriteRule):
    """Rewrite type annotation for SSAValue with constant `IList`
    in `Hinted` type. This should be run after constant folding and
    `WrapConst` rule.
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if isinstance(node, Constant):
            return self.rewrite_Constant(node)

        has_done_something = False
        for result in node.results:
            if not isinstance(hint := result.hints.get("const"), const.Value):
                continue

            typ = result.type
            data = hint.data
            if isinstance(typ, types.PyClass) and typ.is_subseteq(types.PyClass(IList)):
                has_done_something = self._rewrite_IList_type(result, data)
            elif isinstance(typ, types.Generic) and typ.body.is_subseteq(
                types.PyClass(IList)
            ):
                has_done_something = self._rewrite_IList_type(result, data)
        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Constant(self, node: Constant) -> RewriteResult:
        if isinstance(node.value, ir.PyAttr) and isinstance(node.value.data, list):
            stmt = Constant(value=IList(data=node.value.data))
            node.replace_by(stmt)
            self._rewrite_IList_type(stmt.result, node.value.data)
            return RewriteResult(has_done_something=True)
        return RewriteResult()

    def _rewrite_IList_type(self, result: ir.SSAValue, data):
        if not isinstance(data, IList):
            return False

        if not data.data:
            return False

        elem_type = types.PyClass(type(data[0]))
        for elem in data.data[1:]:
            elem_type = elem_type.join(types.PyClass(type(elem)))

        result.type = IListType[elem_type, types.Literal(len(data.data))]
        result.hints["const"] = const.Value(data)
        return True
