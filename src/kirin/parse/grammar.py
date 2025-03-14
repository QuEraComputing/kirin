import textwrap
from typing import Any, Generic, TypeVar, ClassVar
from dataclasses import field, dataclass

import lark

from kirin import ir, types
from kirin.idtable import IdTable
from kirin.ir.traits import LarkLoweringTrait
from kirin.exceptions import LarkLoweringError
from kirin.lowering.state import LoweringState
from kirin.lowering.result import Result

SSA_IDENTIFIER: str = "ssa_identifier"
BLOCK_IDENTIFIER: str = "block_identifier"
BLOCK: str = "block"
REGION: str = "region"
SIGNATURE: str = "signature"
TYPE: str = "type"
DIALECT: str = "dialect"
ATTR: str = "attr"

NodeType = TypeVar("NodeType", bound=ir.Statement | ir.Attribute | None)


@dataclass
class LarkLowerResult:
    result: Any = None

    def expect_none(self):
        if self.result is not None:
            raise LarkLoweringError(f"Expected None, got {self.result}")

    def expect_stmt(self) -> ir.Statement:
        if not isinstance(self.result, ir.Statement):
            raise LarkLoweringError(f"Expected statement, got {self.result}")

        return self.result

    def expect_attr(self) -> ir.Attribute:
        if not isinstance(self.result, ir.Attribute):
            raise LarkLoweringError(f"Expected attribute, got {self.result}")

        return self.result

    def expect_ssa(self) -> ir.SSAValue:
        if not isinstance(self.result, ir.SSAValue):
            raise LarkLoweringError(f"Expected SSA, got {self.result}")

        return self.result


@dataclass
class LarkTraitWrapper(Generic[NodeType]):
    node_type: type[NodeType]
    trait: LarkLoweringTrait[NodeType]

    def lower(
        self, parser: "DialectGroupParser", state: LoweringState, tree: lark.Tree
    ):
        return self.trait.lower(parser, state, self.node_type, tree)


@dataclass
class Grammar:
    rule_ids: IdTable[type[ir.Statement | ir.Attribute] | types.PyClass] = field(
        default_factory=IdTable, init=False
    )
    stmt_ids: list[str] = field(default_factory=list, init=False)
    attr_ids: list[str] = field(default_factory=list, init=False)

    rules: list[str] = field(default_factory=list, init=False)
    stmt_traits: dict[str, LarkTraitWrapper[ir.Statement]] = field(
        default_factory=dict, init=False
    )
    attr_traits: dict[str, LarkTraitWrapper[ir.Attribute] | types.PyClass] = field(
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
        block: block_identifier block_args  newline (stmt newline)*
        stmt_ssa_args: "(" ssa_assign ("," ssa_assign)* ")" | "(" ")"
        stmt_attr_args: "{" attr_assign (",", attr_assign)* "}"

        stmt = {stmt_rule}
        attr = {attr_rule}

        block_identifier: "^" INT
        ssa_assign: IDENTIFIER "=" ssa_identifier
        attr_assign: IDENTIFIER "=" attr
        block_args: '(' ssa_identifier (',' ssa_identifier)* ')'
        ssa_identifier: '%' (IDENTIFIER | INT)
        newline: NEWLINE | "//" NEWLINE | "//" /.+/ NEWLINE
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
        return rule_id, LarkTraitWrapper(node_type, trait)

    def add_stmt(self, node_type: type[ir.Statement]) -> str:
        trait: LarkLoweringTrait[ir.Statement] = node_type.get_trait(LarkLoweringTrait)

        if trait is None:
            raise LarkLoweringError(
                f"Statement {node_type} does not have a LarkLoweringTrait"
            )

        self.stmt_ids(rule_id := self.rule_ids[node_type])
        self.rules.append(f"{rule_id}: {trait.lark_rule(self, node_type)}")
        return rule_id, LarkTraitWrapper(node_type, trait)

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


@dataclass(init=False)
class DialectGroupParser:
    dialects: ir.DialectGroup
    lark_parser: lark.Lark
    stmt_registry: dict[str, LarkTraitWrapper[ir.Statement]]
    attr_registry: dict[str, LarkTraitWrapper[ir.Attribute] | types.PyClass]
    state: LoweringState | None = None

    def __init__(self, dialects: ir.DialectGroup, start_node: ir.Statement):
        self.dialects = dialects

        start = None
        grammer = Grammar()

        for dialect in dialects.data:
            for attr in dialect.attrs:
                rule_id, trait = grammer.add_attr(attr)
                self.attr_traits[rule_id] = trait

            for type_binding in dialect.python_types.keys():
                rule_id = grammer.add_pyclass(type_binding)
                self.attr_traits[rule_id] = type_binding

            for stmt in dialect.stmts:
                rule_id, trait = grammer.add_attr(attr)
                self.stmt_registry[rule_id] = trait

                if stmt is start_node:
                    start = rule_id

        if start is None:
            raise LarkLoweringError(f"Start node {start_node} is not in the dialects")

        self.lark_parser = lark.Lark(grammer.emit(), start=start)

    def visit(self, tree: lark.Tree) -> Result:
        node_type = tree.data
        visitor = getattr(self, f"visit_{node_type}", self.default_visit)
        return visitor(tree)

    def default_visit(self, tree: lark.Tree):
        raise LarkLoweringError(f"Unknown node type {tree.data}")

    def visit_region(self, tree: lark.Tree):
        for child in tree.children:
            self.visit(child)
        return Result()

    def visit_block(self, tree: lark.Tree):
        block = self.state.current_frame.curr_block

        block_args = tree.children[1]
        assert block_args.data == "block_args"
        for arg in block_args.children:
            block.args.append(self.visit(arg).expect_one())

        for stmt in tree.children[2:]:
            self.visit(stmt)

        self.state.current_frame.append_block()
        return Result()

    def visit_stmt(self, tree: lark.Tree):
        if tree.data not in self.stmt_registry:
            raise LarkLoweringError(f"Unknown statement type {tree.data}")

        stmt = self.stmt_registry[tree.data].lower(self, self.state, tree).expect_stmt()
        self.state.current_frame.append_stmt(stmt)

        return Result()

    def visit_attr(self, tree: lark.Tree):
        if tree.data not in self.attr_registry:
            raise LarkLoweringError(f"Unknown statement type {tree.data}")

        reg_result = self.attr_registry[tree.data]
        if isinstance(reg_result, types.PyClass):
            return LarkLowerResult(reg_result)
        else:
            return reg_result.lower(self, self.state, tree)

    def visit_stmt_ssa_args(self, tree: lark.Tree):
        return Result([self.visit(child).expect_one() for child in tree.children])

    def visit_ssa_assign(self, tree: lark.Tree):
        return Result([self.visit(tree.children[1]).expect_one()])

    def visit_ssa_identifier(self, tree: lark.Tree):


    def run(self, body: str, entry: type[NodeType]) -> NodeType:
        raise NotImplementedError("TODO: implement run method")
