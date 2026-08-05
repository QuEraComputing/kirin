from __future__ import annotations

from typing import TypeVar

from kirin import ir, interp, lattice

dialect = ir.Dialect("ssacfg")


@dialect.register(key="main")
class Concrete(interp.MethodTable):

    @interp.impl(ir.SSACFG())
    def ssacfg(self, interp_: interp.Interpreter, frame: interp.Frame, node: ir.Region):
        block = node.blocks[0]
        block_inputs = frame.get_values(block.args)
        while block is not None:
            frame.current_block = block
            frame.set_values(block.args, block_inputs)
            for stmt in block.stmts:
                frame.current_stmt = stmt
                stmt_results = interp_.frame_eval(frame, stmt)
                match stmt_results:
                    case tuple():
                        frame.set_values(stmt._results, stmt_results)
                    case None:
                        continue
                    case interp.Successor(block, block_inputs):
                        pass
                    case interp.ReturnValue():
                        return stmt_results  # terminate the call frame
                    case interp.YieldValue(values):
                        return values  # terminate the region
        return


@dialect.register(key="abstract")
class Abstract(interp.MethodTable):
    """Evaluate an SSACFG region with a dependency-aware worklist.

    The worklist contains path-sensitive :class:`Successor` values: a destination
    block paired with the abstract values passed to its block arguments. Each
    popped successor is checked against this visitation key::

        (successor, generations[successor.block])

    The generation is a per-block invalidation counter. Suppose ``^2`` has
    already been evaluated through ``Successor(^2, True)``. If an SSA value read
    by ``^2`` changes, the solver increments ``generations[^2]`` and requeues the
    same successor. Although its explicit block arguments have not changed, the
    new generation gives it a distinct visitation key::

        (Successor(^2, True), 0)  # original evaluation
        (Successor(^2, True), 1)  # reevaluation after invalidation

    Dependencies come from a static SSA use-def map, with nested uses attributed
    to their enclosing SSACFG block. A dependency change only requeues blocks
    already reached through normal control flow.

    The generation is read when work is popped; it is not stored in the queued
    successor. If several invalidations are pending for one block, every queued
    copy therefore observes the latest generation. The first copy is evaluated
    and the remaining copies match its visitation key and are skipped.

    Unreached blocks are never scheduled by dependency changes, and the existing
    per-block visitation limit remains the convergence guard.
    """

    FrameType = TypeVar("FrameType", bound=interp.AbstractFrame)
    LatticeType = TypeVar("LatticeType", bound=lattice.BoundedLattice)

    @interp.impl(ir.SSACFG())
    def ssacfg(
        self,
        interp_: interp.AbstractInterpreter[FrameType, LatticeType],
        frame: FrameType,
        node: ir.Region,
    ):
        result = None
        dependents = self._get_dependents(node)
        reached: set[ir.Block] = set()
        generations: dict[ir.Block, int] = {}
        frame.worklist.append(
            interp.Successor(node.blocks[0], *frame.get_values(node.blocks[0].args))
        )
        while (succ := frame.worklist.pop()) is not None:
            visited = frame.visited.setdefault(succ.block, set())
            generation = generations.get(succ.block, 0)
            visit = (succ, generation)
            if visit in visited:
                continue

            reached.add(succ.block)
            block_result, changes = self.run_succ(interp_, frame, succ)
            if len(frame.visited[succ.block]) < 128:
                frame.visited[succ.block].add(visit)
            else:
                continue

            for value in changes:
                for block in dependents.get(value, ()):
                    if block is succ.block or block not in reached:
                        continue
                    generations[block] = generations.get(block, 0) + 1
                    frame.worklist.append(
                        interp.Successor(block, *frame.get_values(block.args))
                    )

            if isinstance(block_result, interp.Successor):
                raise interp.InterpreterError(
                    "unexpected successor, successors should be in worklist"
                )

            result = interp_.join_results(result, block_result)

        if isinstance(result, interp.YieldValue):
            return result.values
        return result

    @staticmethod
    def _get_dependents(node: ir.Region) -> dict[ir.SSAValue, set[ir.Block]]:
        dependents: dict[ir.SSAValue, set[ir.Block]] = {}
        for stmt in node.walk():
            block = Abstract._get_enclosing_block(node, stmt)
            if block is None:
                continue
            for value in stmt.args:
                dependents.setdefault(value, set()).add(block)
        return dependents

    @staticmethod
    def _get_enclosing_block(region: ir.Region, stmt: ir.Statement) -> ir.Block | None:
        node: ir.IRNode | None = stmt
        while node is not None:
            parent = node.parent_node
            if isinstance(parent, ir.Block) and parent.parent_node is region:
                return parent
            node = parent
        return None

    def run_succ(
        self,
        interp_: interp.AbstractInterpreter[FrameType, LatticeType],
        frame: FrameType,
        succ: interp.Successor,
    ) -> tuple[interp.SpecialValue[LatticeType], set[ir.SSAValue]]:
        frame._take_changes()
        frame.current_block = succ.block
        frame.set_values(succ.block.args, succ.block_args)
        for stmt in succ.block.stmts:
            frame.current_stmt = stmt
            stmt_results = interp_.frame_eval(frame, stmt)
            if isinstance(stmt_results, tuple):
                frame.set_values(stmt._results, stmt_results)
            elif stmt_results is None:
                continue  # empty result
            else:  # terminate
                return stmt_results, frame._take_changes()
        return None, frame._take_changes()
