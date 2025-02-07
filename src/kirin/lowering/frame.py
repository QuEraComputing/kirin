import ast
from typing import TYPE_CHECKING, Any, TypeVar, Callable, Optional, Sequence
from dataclasses import field, dataclass

from kirin.ir import Block, Region, SSAValue, Statement
from kirin.exceptions import DialectLoweringError
from kirin.lowering.stream import StmtStream

if TYPE_CHECKING:
    from kirin.lowering.state import LoweringState


CallbackFn = Callable[["Frame", SSAValue], SSAValue]


@dataclass
class Frame:
    state: "LoweringState"
    """lowering state"""
    parent: Optional["Frame"]
    """parent frame, if any"""
    stream: StmtStream[ast.stmt]
    """stream of statements to be lowered"""

    curr_region: Region
    """current region being lowered"""
    entr_block: Block
    """entry block of the frame region"""
    curr_block: Block
    """current block being lowered"""
    next_block: Block
    """next block to be lowered, but not yet inserted in the region"""

    # known variables, local SSA values or global values
    defs: dict[str, SSAValue] = field(default_factory=dict)
    """values defined in the current frame"""
    globals: dict[str, Any] = field(default_factory=dict)
    """global values known to the current frame"""
    captures: dict[str, SSAValue] = field(default_factory=dict)
    """values accessed from the parent frame"""
    capture_callback: Optional[CallbackFn] = None
    """callback function that creates a local SSAValue value when an captured value was used."""

    @classmethod
    def from_stmts(
        cls,
        stmts: Sequence[ast.stmt] | StmtStream[ast.stmt],
        state: "LoweringState",
        parent: Optional["Frame"] = None,
        region: Optional[Region] = None,
        entr_block: Optional[Block] = None,
        next_block: Optional[Block] = None,
        globals: dict[str, Any] | None = None,
        capture_callback: Optional[CallbackFn] = None,
    ):
        """Create a new frame from a list of statements or a new `StmtStream`.

        - `stmts`: list of statements or a `StmtStream` to be lowered.
        - `region`: `Region` to append the new block to, `None` to create a new one, default `None`.
        - `entr_block`: `Block` to append the new statements to,
            `None` to create a new one and attached to the region, default `None`.
        - `next_block`: `Block` to use if branching to a new block, if `None` to create
            a new one without attaching to the region. (note: this should not attach to
            the region at frame construction)
        - `globals`: global variables, default `None`.
        """
        if not isinstance(stmts, StmtStream):
            stmts = StmtStream(stmts)

        region = region or Region()

        entr_block = entr_block or Block()
        region.blocks.append(entr_block)

        return cls(
            state=state,
            parent=parent,
            stream=stmts,
            curr_region=region or Region(entr_block),
            entr_block=entr_block,
            curr_block=entr_block,
            next_block=next_block or Block(),
            globals=globals or {},
            capture_callback=capture_callback,
        )

    def get(self, name: str) -> SSAValue | None:
        value = self.get_local(name)
        if value is not None:
            return value

        # NOTE: look up local first, then globals
        if name in self.globals:
            return self.state.visit(ast.Constant(self.globals[name])).expect_one()
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

    def get_scope(self, name: str):
        """Get a variable from current scope.

        Args:
            name(str): variable name

        Returns:
            SSAValue: the value of the variable

        Raises:
            DialectLoweringError: if the variable is not found in the scope,
                or if the variable has multiple possible values.
        """
        value = self.defs.get(name)
        if isinstance(value, SSAValue):
            return value
        else:
            raise DialectLoweringError(f"Variable {name} not found in scope")

    StmtType = TypeVar("StmtType", bound=Statement)

    def append_stmt(self, stmt: StmtType) -> StmtType:
        if not stmt.dialect:
            raise DialectLoweringError(f"unexpected builtin statement {stmt.name}")
        elif stmt.dialect not in self.state.dialects:
            raise DialectLoweringError(
                f"Unsupported dialect `{stmt.dialect.name}` in statement {stmt.name}"
            )
        self.curr_block.stmts.append(stmt)
        stmt.source = self.state.source
        return stmt

    def jump_next(self):
        """Jump to the next block and return it.
        This appends the current `Frame.next_block` to the current region
        and creates a new Block for `next_block`.

        Returns:
            Block: the next block
        """
        block = self.append_block(self.next_block)
        self.next_block = Block()
        return block

    def append_block(self, block: Block | None = None):
        """Append a block to the current region.

        Args:
            block(Block): block to append, default `None` to create a new block.
        """
        block = block or Block()
        self.curr_region.blocks.append(block)
        self.curr_block = block
        return block

    def __repr__(self):
        return f"Frame({len(self.defs)} defs, {len(self.globals)} globals)"
