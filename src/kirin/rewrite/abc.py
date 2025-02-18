from abc import ABC
from dataclasses import dataclass

from kirin.ir import Pure, Block, IRNode, Region, MaybePure, Statement
from kirin.rewrite.result import RewriteResult


@dataclass(repr=False)
class RewriteRule(ABC):
    """A rewrite rule that matches and rewrites IR nodes.

    The rewrite rule is applied to an IR node by calling the instance with the node as an argument.
    The rewrite rule should mutate the node instead of returning a new node. A `RewriteResult` should
    be returned to indicate whether the rewrite rule has done something, whether the rewrite rule
    should terminate, and whether the rewrite rule has exceeded the maximum number of iterations.
    """

    def rewrite(self, node: IRNode) -> RewriteResult:
        if isinstance(node, Region):
            return self.rewrite_Region(node)
        elif isinstance(node, Block):
            return self.rewrite_Block(node)
        elif isinstance(node, Statement):
            return self.rewrite_Statement(node)
        else:
            return RewriteResult()

    def rewrite_Region(self, node: Region) -> RewriteResult:
        return RewriteResult()

    def rewrite_Block(self, node: Block) -> RewriteResult:
        return RewriteResult()

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        return RewriteResult()

    def is_pure(self, node: Statement):
        if node.has_trait(Pure):
            return True

        if (trait := node.get_trait(MaybePure)) and trait.is_pure(node):
            return True
        return False
