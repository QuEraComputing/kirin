import textwrap
from typing import Generic, TypeVar, ClassVar, Optional
from dataclasses import field, dataclass

import lark

from kirin import ir, types, lowering
from kirin.idtable import IdTable
from kirin.dialects import func
from kirin.ir.traits import LarkLoweringTrait
from kirin.exceptions import LarkLoweringError

SSA_IDENTIFIER: str = "ssa_identifier"
BLOCK_IDENTIFIER: str = "block_identifier"
BLOCK: str = "block"
REGION: str = "region"
SIGNATURE: str = "signature"
TYPE: str = "type"
DIALECT: str = "dialect"
ATTR: str = "attr"

NodeType = TypeVar("NodeType", bound=ir.Statement | ir.Attribute | None)

T = TypeVar("T")


@dataclass
class LarkLowerResult(Generic[T]):
    result: T = field(default=None)

    def expect[T](self, typ: type[T]) -> T:
        if not isinstance(self.result, typ):
            raise ValueError(f"Expected {typ}, got {self.result}")

        return self.result


@dataclass
class Grammar:
    rule_ids: IdTable = field(default_factory=IdTable, init=False)
    stmt_ids: list[str] = field(default_factory=list, init=False)
    attr_ids: list[str] = field(default_factory=list, init=False)

    rules: list[str] = field(default_factory=list, init=False)
    stmt_traits: dict[str, LarkLoweringTrait[ir.Statement]] = field(
        default_factory=dict, init=False
    )
    attr_traits: dict[str, LarkLoweringTrait[ir.Attribute] | types.PyClass] = field(
        default_factory=dict, init=False
    )
    type_map: dict[str, type[ir.Statement] | type[ir.Attribute]] = field(
        default_factory=dict, init=False
    )

    header: ClassVar[str] = textwrap.dedent(
        """
        %import common.NEWLINE
        %import common.CNAME -> IDENTIFIER
        %import common.INT
        %import common.FLOAT
        %import common.ESCAPED_STRING -> STRING
        %import common.WS
        %ignore WS
        %ignore "│"

        region: "{{" newline (newline block)* "}}" newline*
        block: block_identifier "(" block_args ")" ":" newline (stmt newline)*
        signature: ( "(" ")" | "(" attr ("," attr)* ")" ) "->" attr
        stmt_ssa_args: "(" kwarg_ssa ("," kwarg_ssa)* ")" | "(" ")"
        stmt_attr_args: "{" kwarg_attr (",", kwarg_attr)* "}"
        stmt_return_args: ssa_value ("," ssa_value)*
        block_args:  block_argument ("," block_argument)*

        block_identifier: "^" INT
        kwarg_ssa: IDENTIFIER "=" ssa_value
        kwarg_attr: IDENTIFIER "=" attr
        block_argument: ssa_identifier | annotated_ssa_identifier
        annotated_ssa_identifier: ssa_identifier ":" attr

        ssa_identifier: '%' (IDENTIFIER | INT)
        ssa_value: '%' (IDENTIFIER | INT)
        newline: NEWLINE | "//" NEWLINE | "//" /.+/ NEWLINE

        stmt = {stmt_rule}
        attr = {attr_rule}
        """
    )

    def add_attr(self, node_type: type[ir.Attribute]) -> str:
        trait: LarkLoweringTrait[ir.Attribute] = node_type.get_trait(LarkLoweringTrait)

        if trait is None:
            raise LarkLoweringError(
                f"Attribute {node_type} does not have a LarkLoweringTrait"
            )

        self.attr_ids(rule_id := self.rule_ids[node_type])
        self.rules.append(f"{rule_id}: {trait.lark_rule(self, node_type)}")
        self.type_map[rule_id] = node_type
        return rule_id, trait

    def add_stmt(self, node_type: type[ir.Statement]) -> str:
        trait: LarkLoweringTrait[ir.Statement] = node_type.get_trait(LarkLoweringTrait)

        if trait is None:
            raise LarkLoweringError(
                f"Statement {node_type} does not have a LarkLoweringTrait"
            )

        self.stmt_ids(rule_id := self.rule_ids[node_type])
        self.rules.append(f"{rule_id}: {trait.lark_rule(self, node_type)}")
        self.type_map[rule_id] = node_type
        return rule_id, node_type

    def add_pyclass(self, node: types.PyClass) -> str:
        rule = f'"{node.prefix}.{node.display_name}"'
        self.attr_ids(rule_id := self.rule_ids[node])
        self.rules.append(f"{rule_id}: {rule}")
        return rule_id, node

    def emit(self) -> str:
        stmt = " | ".join(self.stmt_ids)
        attr = " | ".join(self.attr_ids)
        return self.header.format(stmt_rule=stmt, attr_rule=attr) + "\n".join(
            self.rules
        )


@dataclass
class LarkLoweringState:
    dialects: ir.DialectGroup
    source_info: lowering.SourceInfo
    registry: dict[
        str,
        LarkLoweringTrait[ir.Statement]
        | LarkLoweringTrait[ir.Attribute]
        | types.PyClass,
    ]
    type_map: dict[str, type[ir.Statement] | type[ir.Attribute]]

    _current_frame: Optional[lowering.Frame[lark.Tree]] = field(
        default=None, init=False
    )

    @classmethod
    def from_stmt(
        cls,
        stmt: lark.Tree,
        dialect_group_parser: "DialectGroupParser",
    ):
        return cls(
            dialect_group_parser.dialects,
            lowering.SourceInfo.from_lark_tree(stmt),
            registry=dialect_group_parser.registry,
            type_map=dialect_group_parser.type_map,
        )

    @property
    def current_frame(self) -> lowering.Frame[lark.Tree]:
        if self._current_frame is None:
            raise ValueError("No frame")
        return self._current_frame

    @property
    def code(self):
        stmt = self.current_frame.curr_region.blocks[0].first_stmt
        if stmt:
            return stmt
        raise ValueError("No code generated")

    StmtType = TypeVar("StmtType", bound=ir.Statement)

    def append_stmt(self, stmt: StmtType) -> StmtType:
        """Shorthand for appending a statement to the current block of current frame."""
        return self.current_frame.append_stmt(stmt)

    def push_frame(self, frame: lowering.Frame):
        frame.parent = self._current_frame
        self._current_frame = frame
        return frame

    def pop_frame(self, finalize_next: bool = True):
        """Pop the current frame and return it.

        Args:
            finalize_next(bool): If True, append the next block of the current frame.

        Returns:
            Frame: The popped frame.
        """
        if self._current_frame is None:
            raise ValueError("No frame to pop")
        frame = self._current_frame

        if finalize_next and frame.next_block.parent is None:
            frame.append_block(frame.next_block)
        self._current_frame = frame.parent
        return frame

    def update_lineno(self, node: lark.Tree):
        self.source = lowering.SourceInfo.from_lark_tree(node)

    def visit(self, node: lark.Tree | lark.Token) -> LarkLowerResult:
        if isinstance(node, lark.Tree):
            self.source_info = lowering.SourceInfo.from_lark_tree(node)
            return getattr(self, f"visit_{node.data}", self.default)(node)
        elif isinstance(node, lark.Token):
            self.source_info = lowering.SourceInfo.from_lark_token(node)
            return LarkLowerResult(node.value)
        else:
            raise ValueError(f"Unknown node type {node}")

    def default(self, tree: lark.Tree):
        raise LarkLoweringError(f"Unknown node type {tree.data}")

    def visit_region(self, tree: lark.Tree):
        for child in tree.children:
            self.visit(child)

        return LarkLowerResult(self.current_frame.curr_region)

    def visit_block(self, tree: lark.Tree):
        self.current_frame.append_block()

        for child in tree.children:
            self.visit(child)

        return LarkLowerResult(self.current_frame.curr_block)

    def visit_signature(self, tree: lark.Tree) -> func.Signature:
        *inputs, ret = [
            self.visit(child).expect(types.TypeAttribute)
            for child in tree.children
            if isinstance(child, lark.Tree)
        ]
        return LarkLowerResult(func.Signature(inputs, ret=ret))

    def visit_stmt(self, tree: lark.Tree):
        if tree.data not in self.registry:
            raise LarkLoweringError(f"Unknown statement type {tree.data}")

        return self.registry[tree.data].lower(self, self.type_map[tree.data], tree)

    def visit_attr(self, tree: lark.Tree):
        if tree.data not in self.registry:
            raise LarkLoweringError(f"Unknown statement type {tree.data}")

        reg_result = self.registry[tree.data]
        if isinstance(reg_result, types.PyClass):
            return LarkLowerResult(reg_result)
        else:
            return reg_result.lower(self, self.type_map[tree.data], tree)

    def visit_ssa_stmt_args(self, tree: lark.Tree):
        return LarkLowerResult(
            dict(
                self.visit(child).expect(tuple)
                for child in tree.children
                if isinstance(child, lark.Tree)
            )
        )

    def visit_stmt_attr_args(self, tree: lark.Tree):
        return LarkLowerResult(
            dict(
                self.visit(child).expect(tuple)
                for child in tree.children
                if isinstance(child, lark.Tree)
            )
        )

    def visit_kwarg_ssa(self, tree: lark.Tree):
        name = self.visit(tree.children[0]).expect(str)
        value = self.visit(tree.children[2]).expect(ir.SSAValue)
        return LarkLowerResult((name, value))

    def visit_kwarg_attr(self, tree: lark.Tree):
        name = self.visit(tree.children[0]).expect(str)
        value = self.visit(tree.children[2]).expect(ir.Attribute)
        return LarkLowerResult((name, value))

    def visit_ssa_identifier(self, tree: lark.Tree):
        return LarkLowerResult(
            "".join(str(self.visit(child).result) for child in tree.children)
        )

    def visit_block_identifier(self, tree: lark.Tree):
        return LarkLowerResult(self.visit(tree.children[1]).expect(int))

    def visit_block_argument(self, tree: lark.Tree):
        results = list(map(self.visit, tree.children))

        if len(results) == 2:
            ident = results[0].expect(str)
            attr = types.Any
        elif len(results) == 3:
            ident = results[0].expect(str)
            attr = results[2].expect(ir.Attribute)
        else:
            raise ValueError(f"Expected 2 or 3 results, got {len(results)}")

        assert ident.startswith("%")

        self.current_frame.defs[ident] = (
            block_arg := self.current_frame.curr_block.args.append_from(
                attr, (None if ident[1:].isnumeric() else ident[1:])
            )
        )

        return LarkLowerResult(block_arg)

    def visit_stmt_return_args(self, tree: lark.Tree):
        return LarkLowerResult(
            [
                self.visit(child).expect(str)
                for child in tree.children
                if isinstance(child, lark.Tree)
            ]
        )

    def visit_ssa_value(self, tree: lark.Tree):
        ident = self.visit_ssa_identifier(tree).expect(str)
        return LarkLowerResult(self.current_frame.get_scope(ident))


@dataclass(init=False)
class DialectGroupParser:
    dialects: ir.DialectGroup
    lark_parser: lark.Lark
    registry: dict[
        str,
        LarkLoweringTrait[ir.Statement]
        | LarkLoweringTrait[ir.Attribute]
        | types.PyClass,
    ]
    type_map: dict[str, type[ir.Statement] | type[ir.Attribute]]

    def __init__(self, dialects: ir.DialectGroup, start_node: ir.Statement):
        self.dialects = dialects

        start = None
        grammer = Grammar()

        for dialect in dialects.data:
            for attr in dialect.attrs:
                rule_id, trait = grammer.add_attr(attr)
                self.registry[rule_id] = trait
                self.type_map[rule_id] = attr

            for type_binding in dialect.python_types.keys():
                rule_id = grammer.add_pyclass(type_binding)
                self.registry[rule_id] = type_binding

            for stmt in dialect.stmts:
                rule_id, trait = grammer.add_attr(attr)
                self.registry[rule_id] = trait
                self.type_map[rule_id] = stmt

                if stmt is start_node:
                    start = rule_id

        if start is None:
            raise LarkLoweringError(f"Start node {start_node} is not in the dialects")

        self.lark_parser = lark.Lark(grammer.emit(), start=start)
