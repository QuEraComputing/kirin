import ast
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Callable, Optional, Sequence, TypeVar

from kirin.ir import Block, Region, SSAValue, Statement
from kirin.lowering.stream import StmtStream

if TYPE_CHECKING:
    from kirin.ir import TypeAttribute
    from kirin.lowering.state import LoweringState


CallbackFn = Callable[["Frame", SSAValue], SSAValue]


@dataclass
class Frame:
    state: "LoweringState"
    parent: Optional["Frame"]
    stream: StmtStream

    current_region: Region
    """current region being lowered
    """
    entry_block: Block
    """entry block of the frame
    """
    current_block: Block
    """current block being lowered
    """
    next_block: Block | None = None
    """next block, if any
    """

    # known variables, local SSA values or global values
    defs: dict[str, SSAValue | set[SSAValue]] = field(default_factory=dict)
    """values defined in the current frame
    """
    globals: dict[str, Any] = field(default_factory=dict)
    """global values known to the current frame
    """
    captures: dict[str, SSAValue] = field(default_factory=dict)
    """values accessed from the parent frame
    """
    capture_callback: Optional[CallbackFn] = None
    """callback function that creates a local SSAValue value when an captured value was used.
    """

    @classmethod
    def from_stmts(
        cls,
        stmts: Sequence[ast.stmt] | StmtStream,
        state: "LoweringState",
        parent: Optional["Frame"] = None,
        region: Optional[Region] = None,
        block: Optional[Block] = None,
        globals: dict[str, Any] | None = None,
        capture_callback: Optional[CallbackFn] = None,
    ):
        """Create a new frame from a list of statements or a new `StmtStream`.

        - `stmts`: list of statements or a `StmtStream` to be lowered.
        - `region`: `Region` to append the new block to, `None` to create a new one, default `None`.
        - `block`: `Block` to append the new statements to, `None` to create a new one, default `None`.
        - `globals`: global variables, default `None`.
        """
        if not isinstance(stmts, StmtStream):
            stmts = StmtStream(stmts)

        block = block or Block()
        if region:
            region.blocks.append(block)

        return cls(
            state=state,
            parent=parent,
            stream=stmts,
            current_region=region or Region(block),
            entry_block=block,
            current_block=block,
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
            value = self.defs[name]
            # phi node used first time
            # replace with an argument
            if isinstance(value, set):
                it = iter(value)
                typ = next(it).type
                for v in it:
                    typ: TypeAttribute = v.type.join(typ)  # type: ignore
                ret = self.current_block.args.append_from(typ, name)
                self.defs[name] = ret
                return ret
            else:
                return value

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

    StmtType = TypeVar("StmtType", bound=Statement)

    def append_stmt(self, stmt: StmtType) -> StmtType:
        self.current_block.stmts.append(stmt)
        return stmt

    def append_block(self, block: Block | None = None):
        block = block or Block()
        self.current_region.blocks.append(block)
        self.current_block = block
        return block

    def __repr__(self):
        return f"Frame({len(self.defs)} defs, {len(self.globals)} globals)"
