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


_registry_epoch = 0
"""Bumped whenever a dialect gains a new method table.

`Registry.interpreter` caches the tables it builds on the `DialectGroup`, which
is only sound while no dialect in that group registers new implementations.
Registration normally happens at import time, before any group is built, but
`Dialect.register` may be called at any point, so it bumps this counter to
invalidate every cached table.
"""


def invalidate_interpreter_cache() -> None:
    """Discard every cached interpreter registry.

    Called by `Dialect.register` when a new method table is added.
    """
    global _registry_epoch
    _registry_epoch += 1


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

        Note:
            The returned mapping is cached and shared between callers. Treat it
            as read-only; mutating it would corrupt other interpreters built
            from the same dialect group.
        """
        # Building this table walks every method table of every dialect with
        # `inspect.getmembers`, which is expensive, and
        # `Interpreter.__post_init__` asks for it on every interpreter instance.
        # `Method.__call__` builds a fresh interpreter per call, so calling a
        # kernel from Python in a loop rebuilds the identical table every time.
        # The result depends only on the dialect group and the interpreter keys,
        # so cache it on the group.
        cache_key = tuple(keys)
        cached = self.dialects._interpreter_cache.get(cache_key)
        if cached is not None:
            epoch, table = cached
            if epoch == _registry_epoch:
                return table

        table = self._build_interpreter(cache_key)
        self.dialects._interpreter_cache[cache_key] = (_registry_epoch, table)
        return table

    def _build_interpreter(
        self, keys: tuple[str, ...]
    ) -> dict["Signature", "BoundedDef"]:
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
                            if sig not in registry:
                                registry[sig] = member
        return registry
