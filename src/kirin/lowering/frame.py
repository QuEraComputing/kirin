from __future__ import annotations

from typing import TYPE_CHECKING, Any, Generic, TypeVar, Callable, Optional, overload
from dataclasses import field, dataclass

from kirin.ir import Block, Region, SSAValue, Statement

from .stream import Stmt, StmtStream
from .exception import BuildError

if TYPE_CHECKING:
    from .state import State

CallbackFn = Callable[["Frame", SSAValue], SSAValue]


@dataclass
class Frame(Generic[Stmt]):
    state: State
    """lowering state"""
    parent: Optional[Frame] = field(default=None, init=False)
    """parent frame, if any"""
    stream: StmtStream[Stmt]
    """stream of statements"""

    curr_region: Region
    """the region this frame is generating"""
    entr_block: Block
    """entry block of the frame region"""
    curr_block: Block
    """current block being lowered"""
    next_block: Block
    """next block to be lowered, but not yet inserted in the region"""

    defs: dict[str, SSAValue] = field(default_factory=dict)
    """values defined in the current frame"""
    globals: dict[str, Any] = field(default_factory=dict)
    """global values known to the current frame"""
    captures: dict[str, SSAValue] = field(default_factory=dict)
    """values accessed from the parent frame"""
    capture_callback: Optional[CallbackFn] = None
    """callback function that creates a local SSAValue value when an captured value was used."""

    def __repr__(self):
        return f"Frame({len(self.defs)} defs, {len(self.globals)} globals)"

    StmtType = TypeVar("StmtType", bound=Statement)

    @overload
    def push(self, node: StmtType) -> StmtType: ...

    @overload
    def push(self, node: Block) -> Block: ...

    def push(self, node: StmtType | Block) -> StmtType | Block:
        if isinstance(node, Block):
            return self._push_block(node)
        elif isinstance(node, Statement):
            return self._push_stmt(node)
        else:
            raise BuildError(f"Unsupported type {type(node)} in push()")

    def _push_stmt(self, stmt: StmtType) -> StmtType:
        if not stmt.dialect:
            raise BuildError(f"unexpected builtin statement {stmt.name}")
        elif stmt.dialect not in self.state.parent.dialects:
            raise BuildError(
                f"Unsupported dialect `{stmt.dialect.name}` from statement {stmt.name}"
            )
        self.curr_block.stmts.append(stmt)
        if stmt.source is None:
            stmt.source = self.state.source
        return stmt

    def _push_block(self, block: Block):
        """Append a block to the current region.

        Args:
            block(Block): block to append, default `None` to create a new block.
        """
        self.curr_region.blocks.append(block)
        self.curr_block = block
        if block.source is None:
            block.source = self.state.source
        return block

    def jump_next_block(self):
        """Jump to the next block and return it.
        This appends the current `Frame.next_block` to the current region
        and creates a new Block for `next_block`.

        Returns:
            Block: the next block
        """
        block = self.push(self.next_block)
        self.next_block = Block()
        return block

    def get(self, name: str) -> SSAValue | None:
        value = self.get_local(name)
        if value is not None:
            return value

        # NOTE: look up local first, then globals
        if name in self.globals:
            return self.state.get_literal(self.globals[name])
        return None

    def get_local(self, name: str) -> SSAValue | None:
        if name in self.defs:
            return self.defs[name]

        if self.parent is None:
            return None  # no parent frame, return None

        value = self.parent.get_local(name)
        if value is not None:
            self.captures[name] = value
            if self.capture_callback:
                # whatever generates a local value gets defined
                ret = self.capture_callback(self, value)
                self.defs[name] = ret
                return ret
            return value
        return None

    def __getitem__(self, name: str) -> SSAValue:
        """Get a variable from current scope.

        Args:
            name(str): variable name

        Returns:
            SSAValue: the value of the variable

        Raises:
            lowering.BuildError: if the variable is not found in the scope,
                or if the variable has multiple possible values.
        """
        value = self.defs.get(name)
        if isinstance(value, SSAValue):
            return value
        else:
            raise BuildError(f"Variable {name} not found in scope")

    def exhaust(self):
        """Exhaust the current stream and return the remaining statements."""
        stream = self.stream
        while stream:
            stmt = stream.pop()
            self.state.parent.visit(self.state, stmt)
