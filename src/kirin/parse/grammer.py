import textwrap
from typing import Any, Callable, ClassVar
from dataclasses import field, dataclass

import lark

from kirin import ir, types
from kirin.idtable import IdTable
from kirin.ir.traits import LarkLoweringTrait
from kirin.exceptions import LarkLoweringError
from kirin.lowering.state import LoweringState
from kirin.lowering.result import Result

SSA_IDENTIFIER: str = 'ssa_identifier'
BLOCK_IDENTIFIER: str = 'block_identifier'
BLOCK: str = 'block'
REGION: str = 'region'
SIGNATURE: str = 'signature'
TYPE: str = 'type'
DIALECT: str = 'dialect'
ATTR: str = 'attr'

def _default_grammar_registry():
    return {
        ir.Region: 'region',
        ir.SSAValue : 'ssa_identifier',
        ir.Block: 'block',
        ir.Attribute: 'attr',
        ir.Statement: 'stmt',
        types.TypeAttribute: 'type',
        types.PyClass: 'pytype',
    }


@dataclass
class Grammer:
    HEADER: ClassVar[str] =
    )
    dialect_group: ir.DialectGroup
    base_grammer_rules: dict[Any, str] = field(default_factory=)
    grammar_registry: dict[Any, str] = field(default_factory=dict)


    def get_parser(self, start: str) -> "LarkParser":







@dataclass(init=False)
class LarkParser:
    dialects: ir.DialectGroup
    lark_parser: lark.Lark
    stmt_traits: dict[str, LarkLoweringTrait]
    attr_traits: dict[str, LarkLoweringTrait]
    py_types: dict[str, types.PyClass]



    def __init__(self, dialects: ir.DialectGroup, start_node: ir.Statement):
        self.dialects = dialects

        header = textwrap.dedent("""
        %import common.NEWLINE
        %import common.CNAME -> IDENTIFIER
        %import common.INT
        %import common.FLOAT
        %import common.ESCAPED_STRING -> STRING
        %import common.WS
        %ignore WS
        %ignore "│"

        region: "{{" newline [block*] "}}" newline
        block: block_identifier '(' [ssa_identifier (',' ssa_identifier)*] ')' newline stmts
        signature: '(' [type (',' type )*] ')' '->' type

        stmts: stmt*

        stmt = {stmt_rule}
        type = {type_rule}
        attr = {attr_rule}

        block_identifier: "^" INT
        typed_identifier: ssa_identifier ":" type
        ssa_identifier: '%' (IDENTIFIER | INT) | '%' (IDENTIFIER | INT) ":" type
        ?newline: NEWLINE | (NEWLINE '//' /.+/)
        """)


        base_grammer_rules =  _default_grammar_registry()

        start = None
        rule_table = IdTable[Any](prefix="rule")

        current_rules = dict(base_grammer_rules)

        grammer = []
        stmt_rules = []
        attr_rules = []
        type_rules = []

        for dialect in dialects.data:
            for attr in dialect.attrs:
                lark_trait = attr.get_trait(LarkLoweringTrait)
                if lark_trait is None:
                    raise LarkLoweringError(f"Attribute {attr} does not have a LarkLoweringTrait")

                attr_rule = lark_trait.lark_rule(base_grammer_rules, attr)
                current_rules[attr] = (rule_id := rule_table.add(attr))
                attr_rules.append(rule_id)
                grammer.append(f"{rule_id}: {attr_rule}")
                self.attr_traits[rule_id] = lark_trait

            for (prefix, display_name), type_binding in dialect.python_types.items():
                rule = f'"!" "{prefix}.{display_name}"'
                current_rules[type_binding] = (rule_id := rule_table.add(type_binding))
                type_rules.append(f"{rule_id}: {rule}")
                self.py_types[rule_id] = type_binding


            for stmt in dialect.stmts:
                lark_trait = stmt.get_trait(LarkLoweringTrait)
                if lark_trait is None:
                    raise LarkLoweringError(f"Statement {stmt} does not have a LarkLoweringTrait")

                stmt_rule = lark_trait.lark_rule(base_grammer_rules, stmt)

                if stmt is start_node:
                    start = stmt_rule

                current_rules[stmt] = (rule_id := rule_table.add(stmt))
                stmt_rules.append(rule_id)
                grammer.append(f"{rule_id}: {stmt_rule}")
                self.stmt_traits[rule_id] = lark_trait

        stmt_rule = " | ".join(stmt_rules)
        attr_rule = " | ".join(attr_rules)
        type_rule = " | ".join(type_rules)

        grammer = header + "\n".join(grammer)
        grammer = grammer.format(stmt_rule=stmt_rule, attr_rule=attr_rule, type_rule=type_rule)

        if start is None:
            raise LarkLoweringError(f"Start node {start_node} is not in the dialects")

        self.lark_parser = lark.Lark(grammer, start=start)



    def lower(self, tree: lark.Tree):
        node_type = tree.data
        if node_type == "region":
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


