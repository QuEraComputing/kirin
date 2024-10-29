from dataclasses import dataclass

from kirin import ir
from kirin.analysis.cfg import CFG
from kirin.dialects import cf
from kirin.rewrite import RewriteResult, RewriteRule


@dataclass
class CFGCompactify(RewriteRule):
    cfg: CFG

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if (trait := node.get_trait(ir.SSACFGRegion)) is None:
            return RewriteResult()

        has_done_something = False
        for region in node.regions:
            cfg = trait.get_graph(region)
            result = CFGCompactify(cfg).rewrite(region)
            has_done_something = has_done_something or result.has_done_something

        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Region(self, node: ir.Region) -> RewriteResult:
        has_done_something = False
        for block in node.blocks:
            result = self.rewrite_Block(block)
            has_done_something = has_done_something or result.has_done_something

        # only traverse the first level of stmts
        for block in node.blocks:
            for stmt in block.stmts:
                result = self.rewrite_Statement(stmt)
                has_done_something = has_done_something or result.has_done_something

        return RewriteResult(has_done_something=has_done_something)

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        if node in self.cfg.successors:
            successors = self.cfg.successors[node]
            if len(successors) > 1:
                return RewriteResult()

            # block has only one successor
            # if the terminator is cf.Branch
            # append its successor to this block
            # and remove the terminator, the new
            # terminator will be the successor's terminator
            if node.last_stmt and isinstance(node.last_stmt, cf.Branch):
                terminator = node.last_stmt
                successor = terminator.successor
                # not single edge in CFG
                if len(self.cfg.predecessors[successor]) > 1:
                    return RewriteResult()

                stmt = successor.first_stmt
                while stmt is not None:
                    curr = stmt
                    stmt = stmt.next_stmt
                    curr.detach()
                    node.stmts.append(curr)

                for arg, input in zip(successor.args, terminator.arguments):
                    arg.replace_by(input)
                terminator.delete()
                successor.delete()
        else:
            node.delete()
        return RewriteResult(has_done_something=True)
