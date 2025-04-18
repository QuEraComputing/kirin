from __future__ import annotations

from typing import TYPE_CHECKING, TypeVar
from dataclasses import field, dataclass

from typing_extensions import dataclass_transform

from kirin.ir.nodes import Statement
from kirin.ir.attrs.abc import Attribute

T = TypeVar("T")

if TYPE_CHECKING:
    from kirin.types import PyClass
    from kirin.rewrite.abc import RewriteRule
    from kirin.interp.table import MethodTable
    from kirin.lowering.python.dialect import FromPythonAST


@dataclass
class Rules:
    canonicalize: list[RewriteRule] = field(default_factory=list, init=True)
    """A collection of rules for Canonicalize pass."""
    inference: list[RewriteRule] = field(default_factory=list, init=True)
    """A collection of rules for Inference pass."""


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
    rules: Rules = field(default_factory=Rules, init=True)
    """A collection of rewrite rules for the dialect."""
    python_types: dict[tuple[str, str], "PyClass"] = field(
        default_factory=dict, init=True
    )

    def __post_init__(self) -> None:
        from kirin.lowering.python.dialect import NoSpecialLowering

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
        from kirin.lowering.python.dialect import FromPythonAST

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
                        f"Cannot register {node} to Dialect, key {key} exists in {self}"
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

    def register_py_type(
        self,
        node: type[T] | "PyClass[T]",
        display_name: str | None = None,
        prefix: str = "py",
    ):
        from kirin.ir.attrs.types import PyClass

        if isinstance(node, type):
            node = PyClass(node, display_name=display_name, prefix=prefix)

        if isinstance(node, PyClass):
            if (node.prefix, node.display_name) in self.python_types and (
                other_node := self.python_types[(node.prefix, node.display_name)]
            ) != node:
                raise ValueError(
                    f"Cannot register {node} to Dialect, type {other_node.prefix}.{other_node.display_name} exists for {other_node.typ}"
                )

            self.python_types[(node.prefix, node.display_name)] = node
            return node

        else:
            raise ValueError(f"Cannot register {node} to Dialect")

    def canonicalize(self, rule: type[RewriteRule]) -> type[RewriteRule]:
        """Register a rewrite rule to the canonicalization pass.

        Args:
            rule (RewriteRule): The rewrite rule to register.
        """
        self.rules.canonicalize.append(rule())
        return rule

    def post_inference(self, rule: type[RewriteRule]) -> type[RewriteRule]:
        """Register a rewrite rule to the inference pass.
        Usually, this is used to register a rule that requires
        type inference to be run first.

        Args:
            rule (RewriteRule): The rewrite rule to register.
        """
        self.rules.inference.append(rule())
        return rule
