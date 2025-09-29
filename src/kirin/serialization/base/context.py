from typing import List, TypedDict
from importlib import import_module
from dataclasses import field, dataclass

from kirin import ir, types
from kirin.idtable import IdTable
from kirin.serialization.base.serializationunit import SerializationUnit

PREFIX = "_method_@"
PARAM_SEP = "->"


class MethodSymbolMeta(TypedDict, total=False):
    sym_name: str
    arg_types: List[str]
    ret_type: str


@dataclass
class SerializationContext:
    ssa_idtable: IdTable[ir.SSAValue] = field(default_factory=IdTable[ir.SSAValue])
    stmt_idtable: IdTable[ir.Statement] = field(default_factory=IdTable[ir.Statement])
    blk_idtable: IdTable[ir.Block] = field(default_factory=IdTable[ir.Block])
    region_idtable: IdTable[ir.Region] = field(default_factory=IdTable[ir.Region])

    SSA_Lookup: dict[str, ir.SSAValue] = field(default_factory=dict)
    Statement_Lookup: dict[str, ir.Statement] = field(default_factory=dict)
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
        self.Statement_Lookup.clear()
        self.ssa_idtable.clear()
        self.stmt_idtable.clear()
        self.blk_idtable.clear()
        self.region_idtable.clear()
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


def get_cls_from_name(serUnit: SerializationUnit) -> type:
    if not serUnit.module_name or not serUnit.class_name:
        raise ValueError(
            f"Type {serUnit.module_name} or {serUnit.class_name} cannot be None."
        )
    mod = import_module(serUnit.module_name)
    cls = getattr(mod, serUnit.class_name, None)
    if cls is None:
        if serUnit.class_name == "NoneType" and serUnit.module_name == "builtins":
            return type(None)
        else:
            raise ImportError(
                f"Could not find class {serUnit.class_name} in {serUnit.module_name}"
            )
    return cls
