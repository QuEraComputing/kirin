from __future__ import annotations

from dataclasses import field, dataclass

from kirin import ir
from kirin.rewrite.abc import RewriteRule, RewriteResult

try:
    hash(slice(None))
    _HASHABLE_SLICE = True
except TypeError:
    _HASHABLE_SLICE = False


@dataclass
class Info:
    """An object to hold the comparison information of a statement."""

    head: type[ir.Statement]
    args: tuple[ir.SSAValue, ...]
    attributes: tuple[ir.Attribute, ...]
    successors: tuple[ir.Block, ...]
    regions: tuple[ir.Region, ...]
    _hash: int = field(init=False, repr=False)
    _hashable: bool = field(init=False, repr=False)

    def __post_init__(self):
        if not _HASHABLE_SLICE and any(
            isinstance(attr, ir.PyAttr) and isinstance(attr.data, slice)
            for attr in self.attributes
        ):
            self._hash = id(self)
            self._hashable = False
        else:
            self._hash = hash(
                (id(self.head),)
                + tuple(id(ssa) for ssa in self.args)
                + tuple(hash(attr) for attr in self.attributes)
                + tuple(id(succ) for succ in self.successors)
                + tuple(id(region) for region in self.regions)
            )
            self._hashable = True

    def __hash__(self) -> int:
        return self._hash

    def __eq__(self, other: object) -> bool:
        return self is other or (
            self._hashable
            and isinstance(other, Info)
            and self.head == other.head
            and self.args == other.args
            and self.attributes == other.attributes
            and self.successors == other.successors
            and self.regions == other.regions
        )


@dataclass
class CommonSubexpressionElimination(RewriteRule):

    def rewrite_Block(self, node: ir.Block) -> RewriteResult:
        seen: dict[Info, ir.Statement] = {}

        for stmt in node.stmts:
            if not stmt.has_trait(ir.Pure):
                continue

            if stmt.regions:
                continue

            info = Info(
                head=type(stmt),
                args=tuple(stmt.args),
                attributes=tuple(stmt.attributes.values()),
                successors=tuple(stmt.successors),
                regions=tuple(stmt.regions),
            )
            if info in seen:
                old_stmt = seen[info]
                for result, old_result in zip(stmt._results, old_stmt.results):
                    result.replace_by(old_result)
                stmt.delete()
                return RewriteResult(has_done_something=True)
            else:
                seen[info] = stmt
        return RewriteResult()

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not node.regions:
            return RewriteResult()

        has_done_something = False
        for region in node.regions:
            for block in region.blocks:
                result = self.rewrite_Block(block)
                if result.has_done_something:
                    has_done_something = True

        return RewriteResult(has_done_something=has_done_something)
