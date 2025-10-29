from __future__ import annotations

from abc import ABC
from typing import TypeVar
from contextlib import contextmanager
from dataclasses import field, dataclass

from kirin import ir
from kirin.interp import Frame, abc
from kirin.idtable import IdTable
from kirin.worklist import WorkList

TargetType = TypeVar("TargetType")


@dataclass
class EmitFrame(Frame[TargetType]):
    ssa: IdTable[ir.SSAValue] = field(
        default_factory=lambda: IdTable[ir.SSAValue](prefix="ssa_"),
        init=False,
    )
    block: IdTable[ir.Block] = field(
        default_factory=lambda: IdTable[ir.Block](prefix="block_"),
        init=False,
    )
    _indent: int = field(default=0, init=False)

    @contextmanager
    def indent(self):
        self._indent += 1
        try:
            yield
        finally:
            self._indent -= 1


CodeGenFrameType = TypeVar("CodeGenFrameType", bound=EmitFrame)


@dataclass
class EmitTable(IdTable[ir.Statement]):

    def add(self, value: ir.Statement) -> str:
        id = self.next_id
        if (trait := value.get_trait(ir.SymbolOpInterface)) is not None:
            value_name = trait.get_sym_name(value).unwrap()
            curr_ind = self.name_count.get(value_name, 0)
            suffix = f"_{curr_ind}" if curr_ind != 0 else ""
            self.name_count[value_name] = curr_ind + 1
            name = self.prefix + value_name + suffix
            self.table[value] = name
        else:
            name = f"{self.prefix}{self.prefix_if_none}{id}"
            self.next_id += 1
            self.table[value] = name
        return name

    def __getitem__(self, value: ir.Statement) -> str:
        if value in self.table:
            return self.table[value]
        raise KeyError(f"Symbol {value} not found in SymbolTable")

    def get(self, value: ir.Statement, default: str | None = None) -> str | None:
        if value in self.table:
            return self.table[value]
        return default


@dataclass
class EmitABC(abc.InterpreterABC[CodeGenFrameType, TargetType], ABC):
    callables: EmitTable = field(init=False)
    callable_to_emit: WorkList[ir.Statement] = field(init=False)

    def __init_subclass__(cls) -> None:
        super().__init_subclass__()
        cls.callables = EmitTable(prefix="")
        cls.callable_to_emit = WorkList()
        for each in getattr(cls, "keys", ()):
            if not each.startswith("emit."):
                raise ValueError(f"Key {each} does not start with 'emit.'")

    def run(self, node: ir.Method | ir.Statement):
        if isinstance(node, ir.Method):
            node = node.code

        with self.eval_context():
            self.callables.add(node)
            self.callable_to_emit.append(node)
            while self.callable_to_emit:
                callable = self.callable_to_emit.pop()
                if callable is None:
                    break
                self.eval(callable)
        return
