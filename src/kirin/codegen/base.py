from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, ClassVar, Generic, Iterable, TypeVar

from kirin import ir

if TYPE_CHECKING:
    from kirin.codegen.impl import Signature, StatementImpl

Target = TypeVar("Target")


@dataclass(init=False)
class CodeGen(ABC, Generic[Target]):
    keys: ClassVar[list[str]]
    dialects: ir.DialectGroup
    registry: dict["Signature", "StatementImpl"] = field(init=False, repr=False)

    def __init__(self, dialects: ir.DialectGroup | Iterable[ir.Dialect]):
        if not isinstance(dialects, ir.DialectGroup):
            dialects = ir.DialectGroup(dialects)
        self.dialects = dialects
        self.registry = dialects.registry.codegen(self.keys)

    def emit(self, node) -> Target:
        if isinstance(node, ir.Statement):
            return self.emit_Statement(node)
        elif isinstance(node, ir.Method):
            return self.emit_Method(node)
        elif isinstance(node, ir.Region):
            return self.emit_Region(node)
        elif isinstance(node, ir.Block):
            return self.emit_Block(node)
        elif isinstance(node, ir.Attribute):
            return self.emit_Attribute(node)
        else:
            raise NotImplementedError(
                f"Emit for {node.__class__.__name__} not implemented"
            )

    def emit_Statement(self, stmt: ir.Statement) -> Target:
        sig = self.build_signature(stmt)
        if sig in self.registry:
            return self.registry[sig](self, stmt)
        elif stmt.__class__ in self.registry:
            return self.registry[stmt.__class__](self, stmt)
        return self.emit_Statement_fallback(stmt)

    def emit_Attribute(self, attr: ir.Attribute) -> Target:
        if attr.__class__ in self.registry:
            return self.registry[attr.__class__](self, attr)
        return self.emit_Attribute_fallback(attr)

    def emit_Attribute_fallback(self, attr: ir.Attribute) -> Target:
        raise NotImplementedError(f"Emit for {attr.__class__.__name__} not implemented")

    def emit_Statement_fallback(self, stmt: ir.Statement) -> Target:
        raise NotImplementedError(f"Emit for {stmt.__class__.__name__} not implemented")

    def build_signature(self, stmt: ir.Statement) -> "Signature":
        """build signature for querying the statement implementation."""
        return (stmt.__class__, tuple(arg.type for arg in stmt.args))

    @abstractmethod
    def emit_Method(self, mt: ir.Method) -> Target: ...

    @abstractmethod
    def emit_Region(self, region: ir.Region) -> Target: ...

    @abstractmethod
    def emit_Block(self, block: ir.Block) -> Target: ...
