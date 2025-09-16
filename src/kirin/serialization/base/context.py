from typing import List, TypedDict
from dataclasses import field, dataclass

from kirin import ir, types
from kirin.idtable import IdTable

PREFIX = "_method_@"
PARAM_SEP = "->"


class MethodSymbolMeta(TypedDict, total=False):
    sym_name: str
    arg_types: List[str]
    ret_type: str


@dataclass
class SerializationContext:
    ssa_idtable: IdTable[ir.SSAValue] = field(default_factory=IdTable[ir.SSAValue])
    blk_idtable: IdTable[ir.Block] = field(default_factory=IdTable[ir.Block])
    region_idtable: IdTable[ir.Region] = field(default_factory=IdTable[ir.Region])

    SSA_Lookup: dict[str, ir.SSAValue] = field(default_factory=dict)
    Block_Lookup: dict[str, ir.Block] = field(default_factory=dict)
    Region_Lookup: dict[str, ir.Region] = field(default_factory=dict)

    Method_Symbol: dict[str, MethodSymbolMeta] = field(default_factory=dict)
    Method_Runtime: dict[str, ir.Method] = field(default_factory=dict)

    _block_reference_store: dict[str, ir.Block] = field(
        default_factory=dict[str, ir.Block]
    )

    def clear(self) -> None:
        self.SSA_Lookup.clear()
        self.Block_Lookup.clear()
        self.Region_Lookup.clear()
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


def get_str_from_type(typ: types.TypeAttribute) -> str:
    if isinstance(typ, types.PyClass):
        return typ.typ.__name__
    return "None"


def mangle(
    symbol_name: str | None,
    param_types: tuple[types.TypeAttribute, ...],
    output: types.TypeAttribute | None = None,
) -> str:
    mangled_name = f"{PREFIX}{symbol_name}"
    if param_types:
        for typ in param_types:
            mangled_name += f"{PARAM_SEP}{get_str_from_type(typ)}"
    if output is not None:
        mangled_name += f"{PARAM_SEP}{get_str_from_type(output)}"
    return mangled_name
