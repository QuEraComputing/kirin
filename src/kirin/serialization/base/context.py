from dataclasses import field, dataclass

import kirin.serialization.base.impls as _impls
from kirin import ir
from kirin.symbol_table import SymbolTable
from kirin.ir.attrs.types import TypeAttribute
from kirin.serialization.base.ssa_int_idtable import IntIdTable


class NameMangler:
    PREFIX = "_method_"
    SCOPE_SEP = "@"
    PARAM_SEP = ">"
    TYPE_MAP = {
        "int": "i",
        "str": "s",
        "float": "f",
        "bool": "b",
        "NoneType": "n",
        "list": "l",
        "tuple": "t",
        "dict": "d",
    }

    def mangle(
        self,
        symbol_name: str | None,
        scope: list[str | None] | None,
        param_types: tuple[TypeAttribute, ...],
    ) -> str:
        mangled_name = self.PREFIX
        if scope:
            mangled_name = f"{mangled_name}{self.SCOPE_SEP}{self.SCOPE_SEP.join(scope)}"
        mangled_name = f"{mangled_name}{self.SCOPE_SEP}{symbol_name}"
        if param_types:
            for typ in param_types:
                if typ.__repr__() in self.TYPE_MAP:
                    mangled_name = (
                        f"{mangled_name}{self.PARAM_SEP}{self.TYPE_MAP[typ.__repr__()]}"
                    )
                else:
                    mangled_name = f"{mangled_name}{self.PARAM_SEP}x"
        return mangled_name

    def demangle(self, mangled_name: str) -> str:
        if not mangled_name.startswith(self.PREFIX):
            raise ValueError(f"Invalid mangled name: {mangled_name}")
        # reverse_type_map = {v: k for k, v in self.TYPE_MAP.items()}

        # parts = mangled_name[len(self.PREFIX) :].split(self.SCOPE_SEP)


@dataclass
class SerializationContext:
    ssa_idtable: IntIdTable[ir.SSAValue] = field(
        default_factory=IntIdTable[ir.SSAValue]
    )
    blk_idtable: IntIdTable[ir.Block] = field(default_factory=IntIdTable[ir.Block])
    region_idtable: IntIdTable[ir.Region] = field(default_factory=IntIdTable[ir.Region])
    method_symboltable: SymbolTable = field(default_factory=SymbolTable[str])

    SSA_Lookup: dict[int, ir.SSAValue] = field(default_factory=dict)
    Block_Lookup: dict[int, ir.Block] = field(default_factory=dict)
    Region_Lookup: dict[int, ir.Region] = field(default_factory=dict)
    Method_Lookup: dict[int, ir.Method] = field(default_factory=dict)

    name_mangler = NameMangler()

    def clear(self) -> None:
        self.SSA_Lookup.clear()
        self.Block_Lookup.clear()
        self.Region_Lookup.clear()
        self.ssa_idtable.clear()
        self.blk_idtable.clear()
        self.region_idtable.clear()

    assert _impls

    def register_method_symbol(self, method: ir.Method) -> None:
        scope = list(m.sym_name for m in method.backedges)
        param_type = method.arg_types
        mangled_method_name = self.name_mangler.mangle(
            method.sym_name, scope=scope, param_types=param_type
        )
        if mangled_method_name in self.method_symboltable:
            raise ValueError(f"Method name collision: {mangled_method_name}")
        self.method_symboltable[mangled_method_name] = method.sym_name

    def get_method_by_mangled_name(self, method: ir.Method, mangled_name: str) -> str:
        return self.method_symboltable[mangled_name]
