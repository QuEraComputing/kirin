import textwrap
from typing import ClassVar
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


@dataclass
class Grammar:
    rule_ids: IdTable[type[ir.Statement | ir.Attribute] | types.PyClass] = field(
        default_factory=IdTable, init=False
    )
    stmt_ids: list[str] = field(default_factory=list, init=False)
    attr_ids: list[str] = field(default_factory=list, init=False)
    attr_rules: dict[type[ir.Attribute] | types.PyClass, str] = field(
        default_factory=dict, init=False
    )
    rules: list[str] = field(default_factory=list, init=False)
    stmt_traits: dict[str, LarkLoweringTrait[ir.Statement]] = field(
        default_factory=dict, init=False
    )
    attr_traits: dict[str, LarkLoweringTrait[ir.Attribute] | types.PyClass] = field(
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

        stmt = {stmt_rule}
        attr = {attr_rule}

        block_identifier: "^" INT
        block_args: '(' ssa_identifier (',' ssa_identifier)* ')'
        ssa_identifier: '%' (IDENTIFIER | INT) | '%' (IDENTIFIER | INT) ":" type
        newline: NEWLINE | "//" NEWLINE | "//" /.+/ NEWLINE
        """
    )

    def add_attr(self, node: type[ir.Attribute]) -> str:
        trait: LarkLoweringTrait[ir.Attribute] = node.get_trait(LarkLoweringTrait)

        if trait is None:
            raise LarkLoweringError(
                f"Attribute {node} does not have a LarkLoweringTrait"
            )

        self.attr_ids(rule_id := self.rule_ids[node])
        self.rules.append(f"{rule_id}: {trait.lark_rule(self, node)}")
        return rule_id, trait

    def add_stmt(self, node: type[ir.Statement]) -> str:
        trait: LarkLoweringTrait[ir.Statement] = node.get_trait(LarkLoweringTrait)

        if trait is None:
            raise LarkLoweringError(
                f"Statement {node} does not have a LarkLoweringTrait"
            )

        self.stmt_ids(rule_id := self.rule_ids[node])
        self.rules.append(f"{rule_id}: {trait.lark_rule(self, node)}")
        return rule_id, trait

    def add_pyclass(self, node: types.PyClass) -> str:
        rule = f'"{node.prefix}.{node.display_name}"'
        self.attr_ids(rule_id := self.rule_ids[node])
        self.rules.append(f"{rule_id}: {rule}")
        return rule_id

    def emit(self) -> str:
        stmt = " | ".join(self.stmt_ids)
        attr = " | ".join(self.attr_ids)
        return self.header.format(stmt_rule=stmt, attr_rule=attr) + "\n".join(
            self.rules
        )


@dataclass(init=False)
class LarkParser:
    dialects: ir.DialectGroup
    lark_parser: lark.Lark
    stmt_traits: dict[str, LarkLoweringTrait[ir.Statement]]
    attr_traits: dict[str, LarkLoweringTrait[ir.Attribute] | types.PyClass]
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
                self.stmt_traits[rule_id] = trait

                if stmt is start_node:
                    start = rule_id

        if start is None:
            raise LarkLoweringError(f"Start node {start_node} is not in the dialects")

        self.lark_parser = lark.Lark(grammer.emit(), start=start)

    def lower(self, tree: lark.Tree) -> Result:
        node_type = tree.data

        if node_type == "newline":
            return None
        elif node_type == "region":
            return self.lower_region(tree)
        elif node_type == "block":
            return self.lower_block(tree)
        elif node_type == "stmt":
            return self.lower_stmt(tree)
        elif node_type == "attr":
            return self.lower_attr(tree)
        elif node_type == "type":
            return self.lower_type(tree)
        else:
            raise LarkLoweringError(f"Unknown node type {node_type}")

    def lower_region(self, tree: lark.Tree) -> ir.Region:

        for child in tree.children:
            self.lower(child)

        return Result()

    def lower_block(self, tree: lark.Tree) -> ir.Block:
        block = self.state.current_frame.curr_block

        block_args = tree.children[1]
        assert block_args.data == "block_args"
        for arg in block_args.children:
            block.args.append(self.lower(arg).expect_one())

        for stmt in tree.children[2:]:
            self.lower(stmt)

        self.state.current_frame.curr_block
