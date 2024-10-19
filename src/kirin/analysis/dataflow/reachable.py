from dataclasses import dataclass, field
from typing import Iterable

from kirin.interp import AbstractInterpreter
from kirin.interp.base import InterpResult
from kirin.interp.frame import Frame
from kirin.interp.value import Successor
from kirin.ir import Block, CallableStmtInterface, Dialect, IsTerminator, Method
from kirin.ir.group import DialectGroup
from kirin.ir.nodes.region import Region
from kirin.lattice import EmptyLattice
from kirin.worklist import WorkList


@dataclass
class CFGWorkList(WorkList[Successor]):
    current: Block | None = None
    visited: dict[Block, set[Block]] = field(default_factory=dict)

    def push(self, item: Successor):
        if self.current is None:  # begin of CFG
            self.current = item.block
            return super().push(item)

        neighbors = self.visited.setdefault(self.current, set())
        # NOTE: already visited, won't effect
        # generated CFG result
        if item.block in neighbors:
            return

        neighbors.add(item.block)
        super().push(item)


class ReachableAnalysis(AbstractInterpreter[EmptyLattice, CFGWorkList]):
    keys = [
        "reachibility",
        "typeinfer",
        "typeinfer.default",
    ]  # use typeinfer interpreters

    def __init__(
        self, dialects: DialectGroup | Iterable[Dialect], *, fuel: int | None = None
    ):
        super().__init__(dialects, fuel=fuel)

    def run_analysis(self, mt: Method):
        self.eval(mt, tuple(EmptyLattice() for _ in mt.args))

    @classmethod
    def bottom_value(cls) -> EmptyLattice:
        return EmptyLattice()

    @classmethod
    def default_worklist(cls) -> CFGWorkList:
        return CFGWorkList()

    def prehook_succ(self, frame: Frame, succ: Successor):
        self.worklist.current = succ.block
        self.worklist.visited.setdefault(succ.block, set())

    def run_method_region(
        self, mt: Method, body: Region, args: tuple[EmptyLattice, ...]
    ) -> InterpResult[EmptyLattice]:
        return self.run_ssacfg_region(body, (EmptyLattice(),) + args)

    def run_block(self, frame: Frame, succ: Successor) -> EmptyLattice:
        last_stmt = succ.block.last_stmt
        # NOTE: don't error here, that validator's job
        if last_stmt is None or not last_stmt.has_trait(IsTerminator):
            return self.bottom

        # NOTE: we cannot skip the body, cuz  there are statements
        # with regions that should also be analyzed
        for stmt in succ.block.stmts:
            if stmt.has_trait(CallableStmtInterface):  # contains some region body
                self.run_stmt(stmt, tuple(EmptyLattice() for _ in stmt.args))

        self.run_stmt(last_stmt, tuple(EmptyLattice() for _ in last_stmt.args))
        return self.bottom
