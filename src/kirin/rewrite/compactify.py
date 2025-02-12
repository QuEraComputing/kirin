from dataclasses import dataclass

from kirin import ir
from kirin.dialects import cf
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.analysis.cfg import CFG
from kirin.rewrite.walk import Walk
from kirin.rewrite.chain import Chain
from kirin.rewrite.fixpoint import Fixpoint


@dataclass
class DeadBlock(RewriteRule):
    """Compactify the CFG by removing dead blocks."""

    cfg: CFG

    def rewrite_Region(self, node: ir.Region) -> RewriteResult:
        # remove non-entry blocks that are not reachable from the entry block
        # TODO: check if this region is using SSACFG convention?
        has_done_something = False
        for block in node.blocks[1:]:
            predecessors = self.cfg.predecessors.get(block)
            if not predecessors:  # empty predecessors
                successors = self.cfg.successors.get(block, set())
                for successor in successors:
                    self.cfg.predecessors[successor].discard(block)
                self.cfg.successors.pop(block, None)
                self.cfg.predecessors.pop(block, None)
                block.delete()
                has_done_something = True
        return RewriteResult(has_done_something=has_done_something)


@dataclass
class CFGEdge(RewriteRule):
    """Merge non-branching blocks on the edge of the CFG.

    Example:

        /---> [B] --> [D] --> [E]
    [A]-----> [C] -------------^

    [B] and [D] are non-branching blocks on the same edge. They can be merged into one block.

        /---> [B,D] --> [E]
    [A]-----> [C] -------^
    """

    cfg: CFG

    def rewrite_Region(self, node: ir.Region) -> RewriteResult:
        result = RewriteResult()
        for block in node.blocks:
            result = self.rewrite_Block(block).join(result)
        return result

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        successors = self.cfg.successors.get(node, None)
        if (
            successors is None or len(successors) > 1 or len(successors) == 0
        ):  # multiple outgoing edges
            return RewriteResult()

        successor = next(iter(successors))
        if len(self.cfg.predecessors[successor]) > 1:  # multiple incoming edges
            return RewriteResult()

        if not ((last_stmt := node.last_stmt) and isinstance(last_stmt, cf.Branch)):
            return RewriteResult()

        # merge the two blocks
        for arg, input in zip(successor.args, last_stmt.arguments):
            arg.replace_by(input)
        last_stmt.delete()
        for stmt in successor.stmts:
            stmt.detach()
            node.stmts.append(stmt)
        successor.delete()

        # update the CFG
        new_successors = self.cfg.successors[successor]
        self.cfg.successors[node] = new_successors
        for new_successor in new_successors:
            self.cfg.predecessors[new_successor].discard(successor)
            self.cfg.predecessors[new_successor].add(node)
        del self.cfg.successors[successor]
        del self.cfg.predecessors[successor]  # this is just [node]
        return RewriteResult(has_done_something=True)


class DuplicatedBranch(RewriteRule):
    """Merge duplicated branches into a single branch.

    Example:

        [A]-->[B]
          -----^

    Merge the two branches into one without changing the CFG:

        [A]-->[B]
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if (
            not isinstance(node, cf.ConditionalBranch)
            or node.then_successor is not node.else_successor
        ):
            return RewriteResult()

        for then_x, else_x in zip(node.then_arguments, node.else_arguments):
            if then_x is not else_x:
                return RewriteResult()

        node.replace_by(
            cf.Branch(arguments=node.then_arguments, successor=node.then_successor)
        )
        return RewriteResult(has_done_something=True)


@dataclass
class SkipBlock(RewriteRule):
    """Simplify a block that only contains a branch statement."""

    cfg: CFG

    def rewrite_Region(self, node: ir.Region) -> RewriteResult:
        result = RewriteResult()
        for block in node.blocks:
            result = self.rewrite_Block(block).join(result)
        return result

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        if len(node.stmts) != 1:
            return RewriteResult()

        stmt = node.last_stmt
        if not isinstance(stmt, cf.Branch):
            return RewriteResult()

        has_done_something = False
        predecessors = self.cfg.predecessors.get(node, set())
        # only if there is one predecessor and no uses of the arguments
        if len(predecessors) == 1 and all(
            self.can_skip(stmt, each) for each in node.args
        ):
            has_done_something = self.rewrite_pred(node, stmt, next(iter(predecessors)))
        return RewriteResult(has_done_something=has_done_something)

    def can_skip(self, terminator: cf.Branch, value: ir.SSAValue) -> bool:
        for use in value.uses:
            if use.stmt is terminator:
                continue
            return False
        return True

    def rewrite_pred(
        self, node: ir.Block, node_terminator: cf.Branch, predecessor: ir.Block
    ) -> bool:
        terminator = predecessor.last_stmt
        if isinstance(terminator, cf.Branch):
            return self.rewrite_pred_Branch(
                node, node_terminator, predecessor, terminator
            )
        elif isinstance(terminator, cf.ConditionalBranch):
            return self.rewrite_pred_ConditionalBranch(
                node, node_terminator, predecessor, terminator
            )
        return False

    def rewrite_pred_Branch(
        self,
        node: ir.Block,
        node_terminator: cf.Branch,
        predecessor: ir.Block,
        pred_terminator: cf.Branch,
    ) -> bool:
        ssamap = self._block_inputs(node, pred_terminator.arguments)
        pred_terminator.replace_by(
            cf.Branch(
                # NOTE: the argument can also be SSAs from previous blocks (non-phi)
                arguments=tuple(
                    ssamap.get(arg, arg) for arg in node_terminator.arguments
                ),
                successor=node_terminator.successor,
            )
        )

        self.fix_cfg(predecessor, node, node_terminator.successor)
        return True

    def rewrite_pred_ConditionalBranch(
        self,
        node: ir.Block,
        node_terminator: cf.Branch,
        predecessor: ir.Block,
        pred_terminator: cf.ConditionalBranch,
    ) -> bool:
        then_arguments = pred_terminator.then_arguments
        else_arguments = pred_terminator.else_arguments
        then_successor = pred_terminator.then_successor
        else_successor = pred_terminator.else_successor

        has_done_something = False
        if pred_terminator.then_successor is node:
            ssamap = self._block_inputs(node, pred_terminator.then_arguments)
            then_arguments = tuple(
                ssamap.get(arg, arg) for arg in node_terminator.arguments
            )
            then_successor = node_terminator.successor
            has_done_something = True
            self.fix_cfg(predecessor, node, then_successor)

        if pred_terminator.else_successor is node:
            ssamap = self._block_inputs(node, pred_terminator.else_arguments)
            else_arguments = tuple(
                ssamap.get(arg, arg) for arg in node_terminator.arguments
            )
            else_successor = node_terminator.successor
            has_done_something = True
            self.fix_cfg(predecessor, node, else_successor)

        pred_terminator.replace_by(
            cf.ConditionalBranch(
                cond=pred_terminator.cond,
                then_arguments=then_arguments,
                then_successor=then_successor,
                else_arguments=else_arguments,
                else_successor=else_successor,
            )
        )
        return has_done_something

    def fix_cfg(self, predecessor: ir.Block, node: ir.Block, successor: ir.Block):
        node_pred_succ = self.cfg.successors.setdefault(predecessor, set())
        node_pred_succ.discard(node)
        node_pred_succ.add(successor)

        node_succ_pred = self.cfg.predecessors.setdefault(successor, set())
        node_succ_pred.add(predecessor)

        node_pred = self.cfg.predecessors.setdefault(node, set())
        node_pred.discard(predecessor)

    def _block_inputs(
        self, block: ir.Block, arguments: tuple[ir.SSAValue, ...]
    ) -> dict[ir.SSAValue, ir.SSAValue]:
        return dict(zip(block.args, arguments))


@dataclass
class CompactifyRegion(RewriteRule):
    """Wrapper to share the CFG object with same CFG region."""

    cfg: CFG

    def __init__(self, cfg: CFG):
        self.cfg = cfg
        self.rule = Fixpoint(
            Chain(
                DeadBlock(cfg), Walk(DuplicatedBranch()), SkipBlock(cfg), CFGEdge(cfg)
            )
        )

    def rewrite(self, node: ir.IRNode) -> RewriteResult:
        return self.rule.rewrite(node)


@dataclass
class CFGCompactify(RewriteRule):
    """Compactify the CFG by removing dead blocks and merging blocks
    if the statement uses the SSACFG convention. Do nothing if given
    `ir.Region` or `ir.Block` due to no context of the region.

    To compactify hierarchical CFG, combine this rule with `kirin.rewrite.Walk`
    to recursively apply this rule to all statements.
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        result = RewriteResult()
        if not (trait := node.get_trait(ir.SSACFGRegion)):
            return result

        for region in node.regions:
            cfg = trait.get_graph(region)
            result = CompactifyRegion(cfg).rewrite(region).join(result)
        return result
