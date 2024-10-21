from dataclasses import dataclass
from typing import Generic, TypeVar

from kirin import ir
from kirin.dialects.py import stmts
from kirin.rewrite import RewriteResult, RewriteRule

GetItemLikeStmt = TypeVar("GetItemLikeStmt", bound=ir.Statement)


@dataclass(init=False)
class RewriteGetItem(RewriteRule, Generic[GetItemLikeStmt]):
    target_stmt_type: type[GetItemLikeStmt]
    obj_type: ir.TypeAttribute
    getitem_like: stmts.GetItemLike[GetItemLikeStmt]

    def __init__(self, stmt_type: type[GetItemLikeStmt], obj_type: ir.TypeAttribute):
        trait = stmt_type.get_trait(stmts.GetItemLike)
        if trait is None:
            raise ValueError(f"{stmt_type} does not have GetItemLike trait")

        self.obj_type = obj_type
        self.target_stmt_type = stmt_type
        self.getitem_like = trait

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, stmts.GetItem):
            return RewriteResult()

        if not node.obj.type.is_subtype(self.obj_type):
            return RewriteResult()

        node.replace_by(
            self.getitem_like.new(self.target_stmt_type, node.obj, node.index)
        )
        return RewriteResult(has_done_something=True)
