from typing import TypeVar, Iterable
from dataclasses import field, dataclass

from kirin import ir, interp
from kirin.worklist import WorkList

ValueType = TypeVar("ValueType")

@dataclass
class EmitFrame(interp.Frame[ValueType]):
    worklist: WorkList[interp.Successor] = field(default_factory=WorkList)
    block_ref: dict[ir.Block, ValueType] = field(default_factory=dict)

FrameType = TypeVar("FrameType", bound=EmitFrame)

class EmitABC(interp.BaseInterpreter[FrameType, ValueType]):
    def __init__(
        self,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        bottom: ValueType,
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ): ...
    def run_callable_region(
        self, frame: FrameType, code: ir.Statement, region: ir.Region
    ) -> ValueType | interp.Err[ValueType]: ...
    def run_ssacfg_region(
        self, frame: FrameType, region: ir.Region
    ) -> ValueType | interp.Err[ValueType]: ...
    def emit_attribute(self, attr: ir.Attribute) -> ValueType: ...
    def emit_type_Any(self, attr: ir.types.AnyType) -> ValueType: ...
    def emit_type_Bottom(self, attr: ir.types.BottomType) -> ValueType: ...
    def emit_type_Literal(self, attr: ir.types.Literal) -> ValueType: ...
    def emit_type_Union(self, attr: ir.types.Union) -> ValueType: ...
    def emit_type_TypeVar(self, attr: ir.types.TypeVar) -> ValueType: ...
    def emit_type_Vararg(self, attr: ir.types.Vararg) -> ValueType: ...
    def emit_type_Generic(self, attr: ir.types.Generic) -> ValueType: ...
    def emit_type_Const(self, attr: ir.types.Const) -> ValueType: ...
    def emit_type_PyClass(self, attr: ir.types.PyClass) -> ValueType: ...
    def emit_attribute_fallback(self, attr: ir.Attribute) -> ValueType: ...
    def emit_stmt_begin(self, frame: FrameType, stmt: ir.Statement) -> None: ...
    def emit_stmt_end(self, frame: FrameType, stmt: ir.Statement) -> None: ...
    def emit_block_begin(self, frame: FrameType, block: ir.Block) -> None: ...
    def emit_block_end(self, frame: FrameType, block: ir.Block) -> None: ...
    def emit_block(
        self, frame: FrameType, block: ir.Block
    ) -> interp.MethodResult[ValueType]: ...
