from dataclasses import field, dataclass

from kirin import ir
from kirin.serialization.base.utils.attr import (
    DialectSerializer,
    RuntimeSerializer,
    TypeAttributeSerializer,
)
from kirin.serialization.base.utils.ssa_int_idtable import IntIdTable


@dataclass
class SerializationContext:
    SSA_Lookup: dict[int, ir.SSAValue] = field(default_factory=dict)
    Block_Lookup: dict[int, ir.Block] = field(default_factory=dict)
    Region_Lookup: dict[int, ir.Region] = field(default_factory=dict)
    ssa_idtable: IntIdTable[ir.SSAValue] = field(
        default_factory=IntIdTable[ir.SSAValue]
    )
    blk_idtable: IntIdTable[ir.Block] = field(default_factory=IntIdTable[ir.Block])
    region_idtable: IntIdTable[ir.Region] = field(default_factory=IntIdTable[ir.Region])

    runtime_serializer: RuntimeSerializer = field(default_factory=RuntimeSerializer)
    typeattr_serializer: TypeAttributeSerializer = field(
        default_factory=TypeAttributeSerializer
    )
    dialect_serializer: DialectSerializer = field(default_factory=DialectSerializer)

    def clear(self) -> None:
        self.SSA_Lookup.clear()
        self.Block_Lookup.clear()
        self.Region_Lookup.clear()
        self.ssa_idtable.clear()
        self.blk_idtable.clear()
        self.region_idtable.clear()

    def get_or_assign_ssa_id(self, ssa_id: ir.SSAValue) -> int:
        return self.ssa_idtable[ssa_id]

    def get_or_assign_block_id(self, block_id: ir.Block) -> int:
        return self.blk_idtable[block_id]
