from __future__ import annotations

from typing import (
    TYPE_CHECKING,
    Any,
    Generic,
    Literal,
    Optional,
    Sequence,
    Generator,
    overload,
)
from contextlib import contextmanager
from dataclasses import field, dataclass

from kirin.ir import Block, Region, SSAValue, Statement
from kirin.source import SourceInfo

from .abc import ASTNodeType
from .frame import Frame, CallbackFn
from .stream import StmtStream
from .exception import DialectLoweringError

if TYPE_CHECKING:
    from .abc import LoweringABC


@dataclass
class State(Generic[ASTNodeType]):
    """State of the lowering process.

    This class is used to store the state of the lowering process.
    It contains the current frame, the current block, and
    the current source.
    """

    parent: LoweringABC[ASTNodeType]
    """the parent lowering transform"""
    source: SourceInfo
    "source info of the current node"
    lines: list[str]
    "source lines of the code being lowered"
    lineno_offset: int = 0
    "lineno offset at the beginning of the source"
    col_offset: int = 0
    "column offset at the beginning of the source"
    _current_frame: Frame | None = field(default=None, init=False, repr=False)
    "current frame being lowered"

    def __repr__(self) -> str:
        return f"lowering.State({self.current_frame})"

    @property
    def code(self):
        """Obtain the code generated by the lowering process."""
        region = self.root_frame.curr_region
        if not region.blocks:
            raise ValueError("No block generated")

        stmt = region.blocks[0].first_stmt
        if stmt is None:
            raise ValueError("No code generated")
        return stmt

    @property
    def root_frame(self) -> Frame[ASTNodeType]:
        """root frame of the lowering process"""
        if self._current_frame is None:
            raise ValueError("current frame is None")
        root = self._current_frame
        while root.parent is not None:
            root = root.parent
        return root

    @property
    def current_frame(self) -> Frame[ASTNodeType]:
        """current frame being lowered"""
        if self._current_frame is None:
            raise ValueError("current frame is None")
        return self._current_frame

    @dataclass
    class Result:
        """A proxy object to the result of the lowering process.

        Use `.data` to access the result of the lowering process.
        Use `.expect_one()` to assert that the result is a single value.
        """

        data: tuple[SSAValue, ...]

        def expect_one(self) -> SSAValue:
            if len(self.data) == 1:
                return self.data[0]
            raise DialectLoweringError("expected a value, but got None")

    def lower(self, node: ASTNodeType):
        result = self.parent.visit(self, node)
        if isinstance(result, Statement):
            return self.Result(tuple(result._results))
        elif result is None:
            return self.Result(tuple())
        elif isinstance(result, SSAValue):
            return self.Result((result,))
        return self.Result(result)

    def get_literal(self, value) -> SSAValue:
        return self.parent.lower_literal(value)

    @overload
    def get_global(
        self, node: ASTNodeType, *, no_raise: Literal[True] | bool
    ) -> LoweringABC.Result | None: ...

    @overload
    def get_global(
        self, node: ASTNodeType, *, no_raise: Literal[False] = False
    ) -> LoweringABC.Result: ...

    def get_global(self, node: ASTNodeType, *, no_raise: bool = False):
        """Get the global value of a node.
        Args:
            node (ASTNodeType): the node to get the global value of.
            no_raise (bool): if True, do not raise an exception if the value is not found.
        Returns:
            `LoweringABC.Result`: a proxy object to the global value. `.data` is the
                value, and `.expect(type)` will raise an exception if the value is the expected type.
        """
        if no_raise:
            return self.parent.lower_global_no_raise(self, node)
        return self.parent.lower_global(self, node)

    def push_frame(self, frame: Frame):
        frame.parent = self._current_frame
        self._current_frame = frame
        return frame

    def pop_frame(self, finalize_next: bool = True):
        """Pop the current frame and return it.

        Args:
            finalize_next(bool): If True, append the next block of the current frame.

        Returns:
            Frame: The popped frame.
        """
        if self._current_frame is None:
            raise ValueError("No frame to pop")
        frame = self._current_frame

        if finalize_next and frame.next_block.parent is None:
            frame.push(frame.next_block)
        self._current_frame = frame.parent
        return frame

    @contextmanager
    def frame(
        self,
        stmts: Sequence[ASTNodeType] | StmtStream[ASTNodeType],
        parent: Optional["Frame"] = None,
        region: Optional[Region] = None,
        entr_block: Optional[Block] = None,
        next_block: Optional[Block] = None,
        globals: dict[str, Any] | None = None,
        capture_callback: Optional[CallbackFn] = None,
        finalize_next: bool = True,
    ) -> Generator[Frame[ASTNodeType], Any, None]:
        """Context manager to push a new frame and pop it after the block."""

        if not isinstance(stmts, StmtStream):
            stmts = StmtStream(stmts)

        region = region or Region()

        entr_block = entr_block or Block()
        region.blocks.append(entr_block)

        frame = Frame(
            state=self,
            parent=parent or self.current_frame,
            stream=stmts,
            curr_region=region or Region(entr_block),
            entr_block=entr_block,
            curr_block=entr_block,
            next_block=next_block or Block(),
            globals=globals or self.current_frame.globals,
            capture_callback=capture_callback,
        )
        self.push_frame(frame)
        try:
            yield frame
        finally:
            self.pop_frame(finalize_next)
