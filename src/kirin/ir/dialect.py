from __future__ import annotations

from typing import TYPE_CHECKING, TypeVar
from dataclasses import field, dataclass

from typing_extensions import dataclass_transform

from kirin.ir.nodes import Statement
from kirin.ir.attrs.abc import Attribute

T = TypeVar("T")

if TYPE_CHECKING:
    from kirin.interp.table import MethodTable
    from kirin.lowering.dialect import FromPythonAST


# TODO: add an option to generate default lowering at dialect construction
@dataclass
class Dialect:
    """Dialect is a collection of statements, attributes, interpreters, lowerings, and codegen.

    Example:
        ```python
            from kirin import ir

            my_dialect = ir.Dialect(name="my_dialect")

        ```
    """

    name: str
    """The name of the dialect."""
    stmts: list[type[Statement]] = field(default_factory=list, init=True)
    """A list of statements in the dialect."""
    attrs: list[type[Attribute]] = field(default_factory=list, init=True)
    """A list of attributes in the dialect."""
    interps: dict[str, MethodTable] = field(default_factory=dict, init=True)
    """A dictionary of registered method table in the dialect."""
    lowering: dict[str, FromPythonAST] = field(default_factory=dict, init=True)
    """A dictionary of registered python lowering implmentations in the dialect."""

    def __post_init__(self) -> None:
        from kirin.lowering.dialect import NoSpecialLowering

        self.lowering["default"] = NoSpecialLowering()

    def __repr__(self) -> str:
        return f"Dialect(name={self.name}, ...)"

    def __hash__(self) -> int:
        return hash(self.name)

    @dataclass_transform()
    def register(self, node: type | None = None, key: str | None = None):
        """register is a decorator to register a node to the dialect.

        Args:
            node (type | None): The node to register. Defaults to None.
            key (str | None): The key to register the node to. Defaults to None.

        Raises:
            ValueError: If the node is not a subclass of Statement, Attribute, DialectInterpreter, FromPythonAST, or DialectEmit.

        Example:
            * Register a method table for concrete interpreter (by default key="main") to the dialect:
            ```python
                from kirin import ir

                my_dialect = ir.Dialect(name="my_dialect")

                @my_dialect.register
                class MyMethodTable(ir.MethodTable):
                    ...
            ```

            * Register a method table for the interpreter specified by `key` to the dialect:
            ```python
                from kirin import ir

                my_dialect = ir.Dialect(name="my_dialect")

                @my_dialect.register(key="my_interp")
                class MyMethodTable(ir.MethodTable):
                    ...
            ```


        """
        from kirin.interp.table import MethodTable
        from kirin.lowering.dialect import FromPythonAST

        if key is None:
            key = "main"

        def wrapper(node: type[T]) -> type[T]:
            if issubclass(node, Statement):
                self.stmts.append(node)
            elif issubclass(node, Attribute):
                assert (
                    Attribute in node.__mro__
                ), f"{node} is not a subclass of Attribute"
                setattr(node, "dialect", self)
                assert hasattr(node, "name"), f"{node} does not have a name attribute"
                self.attrs.append(node)
            elif issubclass(node, MethodTable):
                if key in self.interps:
                    raise ValueError(
                        f"Cannot register {node} to Dialect, key {key} exists"
                    )
                self.interps[key] = node()
            elif issubclass(node, FromPythonAST):
                if key in self.lowering:
                    raise ValueError(
                        f"Cannot register {node} to Dialect, key {key} exists"
                    )
                self.lowering[key] = node()
            else:
                raise ValueError(f"Cannot register {node} to Dialect")
            return node

        if node is None:
            return wrapper

        return wrapper(node)
