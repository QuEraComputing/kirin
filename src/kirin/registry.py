from __future__ import annotations

import inspect
from typing import TYPE_CHECKING, Iterable
from dataclasses import dataclass

if TYPE_CHECKING:
    import ast

    from kirin import lowering
    from kirin.ir.group import DialectGroup
    from kirin.lowering import FromPythonAST
    from kirin.interp.table import Signature, BoundedDef
    from kirin.lowering.python.dialect import LoweringTransform


@dataclass
class CallLoweringFunction:
    parent: FromPythonAST
    func: LoweringTransform

    def __call__(
        self, state: "lowering.State[ast.AST]", node: "ast.Call"
    ) -> "lowering.Result":
        return self.func(self.parent, state, node)


@dataclass
class LoweringRegistry:
    ast_table: dict[str, FromPythonAST]
    callee_table: dict[object, CallLoweringFunction]


@dataclass
class Registry:
    """Proxy class to build different registries from a dialect group."""

    dialects: "DialectGroup"
    """The dialect group to build the registry from."""

    def ast(self, keys: Iterable[str]) -> LoweringRegistry:
        """select the dialect lowering interpreters for the given key.

        Args:
            keys (Iterable[str]): the keys to search for in the dialects

        Returns:
            a map of dialects to their lowering interpreters
        """
        ret: dict[str, FromPythonAST] = {}
        callee_table: dict[object, CallLoweringFunction] = {}
        from_ast = None
        for dialect in self.dialects.data:
            for key in keys:
                if key in dialect.lowering:
                    from_ast = dialect.lowering[key]
                    break

            if from_ast is None:
                msg = ",".join(keys)
                raise KeyError(f"Lowering not found for {msg}")

            for name in from_ast.names:
                if name.startswith("lower_Call_"):
                    continue  # ignore the Call nodes
                if name in ret:
                    raise KeyError(f"Lowering {name} already exists")
                ret[name] = from_ast

            for obj, trans in from_ast.callee_table.items():
                if obj in callee_table:
                    raise KeyError(f"Lowering {obj} already exists")
                callee_table[obj] = CallLoweringFunction(from_ast, trans.func)
        return LoweringRegistry(ret, callee_table)

    def interpreter(self, keys: Iterable[str]) -> dict["Signature", "BoundedDef"]:
        """select the dialect interpreter for the given key.

        Args:
            keys (Iterable[str]): the keys to search for in the dialects

        Returns:
            a map of statement signatures to their interpretation functions,
            and a map of dialects to their fallback interpreters.
        """
        from kirin.interp.table import BoundedDef

        registry: dict[Signature, BoundedDef] = {}
        for dialect in self.dialects.data:
            dialect_table = None
            for key in keys:
                if key not in dialect.interps:
                    continue

                dialect_table = dialect.interps[key]
                for _, member in inspect.getmembers(dialect_table):
                    if isinstance(member, BoundedDef):
                        for sig in member.signature:
                            registry[sig] = member
                break
        return registry
