from dataclasses import field, dataclass

import kirin.serialization.base.impls as _impls
from kirin import ir
from kirin.serialization.base.ssa_int_idtable import IntIdTable


@dataclass
class SerializationContext:
    ssa_idtable: IntIdTable[ir.SSAValue] = field(
        default_factory=IntIdTable[ir.SSAValue]
    )
    blk_idtable: IntIdTable[ir.Block] = field(default_factory=IntIdTable[ir.Block])
    region_idtable: IntIdTable[ir.Region] = field(default_factory=IntIdTable[ir.Region])

    SSA_Lookup: dict[int, ir.SSAValue] = field(default_factory=dict)
    Block_Lookup: dict[int, ir.Block] = field(default_factory=dict)
    Region_Lookup: dict[int, ir.Region] = field(default_factory=dict)

    Method_SymbolTable: dict[str, ir.Method] = field(
        default_factory=dict[str, ir.Method]
    )

    def clear(self) -> None:
        self.SSA_Lookup.clear()
        self.Block_Lookup.clear()
        self.Region_Lookup.clear()
        self.ssa_idtable.clear()
        self.blk_idtable.clear()
        self.region_idtable.clear()

    assert _impls
