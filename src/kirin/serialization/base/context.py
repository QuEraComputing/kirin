from dataclasses import field, dataclass

from kirin import ir
from kirin.idtable import IdTable


@dataclass
class SerializationContext:
    # IdTable returns human-readable string names (e.g. "%0", "%bb1")
    ssa_idtable: IdTable[ir.SSAValue] = field(default_factory=IdTable[ir.SSAValue])
    blk_idtable: IdTable[ir.Block] = field(default_factory=IdTable[ir.Block])
    region_idtable: IdTable[ir.Region] = field(default_factory=IdTable[ir.Region])

    # lookups are keyed by the string name produced by the IdTable
    SSA_Lookup: dict[str, ir.SSAValue] = field(default_factory=dict[str, ir.SSAValue])
    Block_Lookup: dict[str, ir.Block] = field(default_factory=dict[str, ir.Block])
    Region_Lookup: dict[str, ir.Region] = field(default_factory=dict[str, ir.Region])

    Method_Symbol: dict[str, str] = field(default_factory=dict[str, str])
    Method_Runtime: dict[str, ir.Method] = field(default_factory=dict[str, ir.Method])

    _block_reference_store: dict[str, ir.Block] = field(
        default_factory=dict[str, ir.Block]
    )

    def clear(self) -> None:
        # clear lookups
        self.SSA_Lookup.clear()
        self.Block_Lookup.clear()
        self.Region_Lookup.clear()
        # reset IdTable internals
        for tbl in (self.ssa_idtable, self.blk_idtable, self.region_idtable):
            if hasattr(tbl, "table"):
                tbl.table.clear()
            if hasattr(tbl, "name_count"):
                tbl.name_count.clear()
            if hasattr(tbl, "next_id"):
                tbl.next_id = 0
        self._block_reference_store.clear()
        self.Method_Symbol.clear()
        self.Method_Runtime.clear()

    # def ssa_name(self, val: ir.SSAValue) -> str:
    #     return self.ssa_idtable[val]

    # def register_ssa(self, name: str, val: ir.SSAValue) -> None:
    #     self.SSA_Lookup[name] = val

    # def block_name(self, blk: ir.Block) -> str:
    #     return self.blk_idtable[blk]

    # def register_block(self, name: str, blk: ir.Block) -> None:
    #     self.Block_Lookup[name] = blk

    # def region_name(self, region: ir.Region) -> str:
    #     return self.region_idtable[region]

    # def register_region(self, name: str, region: ir.Region) -> None:
    #     self.Region_Lookup[name] = region
