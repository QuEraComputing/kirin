from importlib import import_module
from dataclasses import field, dataclass

from kirin import ir, types
from kirin.idtable import IdTable
from kirin.serialization.core.serializationunit import SerializationUnit


@dataclass
class TypeAttributeSerializationEntry:
    owner: types.TypeAttribute
    unit: SerializationUnit


@dataclass
class SerializationContext:
    ssa_idtable: IdTable[ir.SSAValue] = field(default_factory=IdTable[ir.SSAValue])
    stmt_idtable: IdTable[ir.Statement] = field(default_factory=IdTable[ir.Statement])
    blk_idtable: IdTable[ir.Block] = field(default_factory=IdTable[ir.Block])
    region_idtable: IdTable[ir.Region] = field(default_factory=IdTable[ir.Region])
    type_attribute_idtable: IdTable[int] = field(
        default_factory=lambda: IdTable[int](prefix="t")
    )
    method_idtable: IdTable[int] = field(
        default_factory=lambda: IdTable[int](prefix="m")
    )

    Dialect_Lookup: dict[str, ir.Dialect] = field(default_factory=dict)
    SSA_Lookup: dict[str, ir.SSAValue] = field(default_factory=dict)
    TypeAttribute_Lookup: dict[str, types.TypeAttribute] = field(default_factory=dict)
    Statement_Lookup: dict[str, ir.Statement] = field(default_factory=dict)
    Block_Lookup: dict[str, ir.Block] = field(default_factory=dict)
    Region_Lookup: dict[str, ir.Region] = field(default_factory=dict)
    Method_Lookup: dict[str, ir.Method] = field(default_factory=dict)

    type_attribute_entries: dict[int, TypeAttributeSerializationEntry] = field(
        default_factory=dict
    )
    TypeAttribute_Definitions: dict[str, SerializationUnit] = field(
        default_factory=dict
    )

    _block_reference_store: dict[str, ir.Block] = field(
        default_factory=dict[str, ir.Block]
    )

    def clear(self) -> None:
        self.SSA_Lookup.clear()
        self.TypeAttribute_Lookup.clear()
        self.Block_Lookup.clear()
        self.Region_Lookup.clear()
        self.Statement_Lookup.clear()
        self.Method_Lookup.clear()
        self.ssa_idtable.clear()
        self.stmt_idtable.clear()
        self.blk_idtable.clear()
        self.region_idtable.clear()
        self.type_attribute_idtable.clear()
        self.method_idtable.clear()
        self._block_reference_store.clear()
        self.Dialect_Lookup.clear()
        self.type_attribute_entries.clear()
        self.TypeAttribute_Definitions.clear()


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
