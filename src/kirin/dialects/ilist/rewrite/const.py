from kirin import ir, types
from kirin.analysis import const
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.py.constant import Constant

from ..stmts import IListType
from ..runtime import IList
from .._dialect import dialect


@dialect.post_inference
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
                has_done_something = has_done_something or self._rewrite_IList_type(
                    result, data
                )
            elif isinstance(typ, types.Generic) and typ.body.is_subseteq(
                types.PyClass(IList)
            ):
                has_done_something = has_done_something or self._rewrite_IList_type(
                    result, data
                )
        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Constant(self, node: Constant) -> RewriteResult:
        if not isinstance(node.value, ir.PyAttr):
            return RewriteResult()

        if isinstance(data := node.value.data, list):
            stmt = Constant(value=IList(data=data))
            node.replace_by(stmt)
            self._rewrite_IList_type(stmt.result, data)
            return RewriteResult(has_done_something=True)
        elif isinstance(data, range):
            new_constant = IList(data=data, elem=types.Int)
            stmt = Constant(value=new_constant)
            # specializing the type computation since we know that a
            # range will always be integer typed.
            stmt.result.hints["const"] = const.Value(new_constant)
            stmt.result.type = IListType[types.Int, types.Literal(len(data))]
            node.replace_by(stmt)
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

        new_type = IListType[elem_type, types.Literal(len(data.data))]
        new_hint = const.Value(data)

        # Check if type and hint are already correct
        if result.type == new_type and result.hints.get("const") == new_hint:
            return False

        result.type = new_type
        result.hints["const"] = new_hint
        return True
