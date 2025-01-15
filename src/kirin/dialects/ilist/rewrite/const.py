from kirin import ir
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule
from kirin.rewrite.result import RewriteResult

from ..stmts import IListType
from ..runtime import IList


class RewriteHinted(RewriteRule):
    """Rewrite type annotation for SSAValue with constant `IList`
    in `Hinted` type. This should be run after constant folding and
    `WrapConst` rule.
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        has_done_something = False
        for result in node.results:
            if not isinstance(result.type, ir.types.Hinted):
                continue
            if not isinstance(result.type.data, const.Value):
                continue
            typ = result.type.type
            data = result.type.data.data
            if isinstance(typ, ir.types.PyClass) and typ.is_subseteq(
                ir.types.PyClass(IList)
            ):
                has_done_something = self._rewrite_IList_type(result, data)
            elif isinstance(typ, ir.types.Generic) and typ.body.is_subseteq(
                ir.types.PyClass(IList)
            ):
                has_done_something = self._rewrite_IList_type(result, data)
        return RewriteResult(has_done_something=has_done_something)

    def _rewrite_IList_type(self, result: ir.SSAValue, data):
        if not isinstance(data, IList):
            return False

        if not data.data:
            return False

        elem_type = ir.types.PyClass(type(data[0]))
        for elem in data.data[1:]:
            elem_type = elem_type.join(ir.types.PyClass(type(elem)))

        result.type = ir.types.Hinted(
            IListType[elem_type, ir.types.Literal(len(data.data))], data
        )
        return True
