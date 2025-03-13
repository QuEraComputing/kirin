from kirin.decl import fields
from kirin.exceptions import LarkLoweringError
from kirin.ir.nodes.stmt import Statement
from kirin.parse.grammar import Grammar, LarkParser
from kirin.lowering.state import LoweringState
from kirin.lowering.result import Result as Result

from ..abc import LarkLoweringTrait


class FromLark(LarkLoweringTrait):
    def lark_rule(
        self,
        grammar: Grammar,
        stmt_type: type[Statement],
    ) -> str:
        assert (
            stmt_type.dialect is not None
        ), f"Statement {stmt_type} must have a dialect"

        stmt_fields = fields(stmt_type)

        if len(stmt_fields.regions) > 0:
            raise LarkLoweringError(
                f"Statement {stmt_type} has regions, which are not supported by FromLark trait. create a custom trait for this statement"
            )

        if len(stmt_fields.blocks) > 0:
            raise LarkLoweringError(
                f"Statement {stmt_type} has blocks, which are not supported by FromLark trait. create a custom trait for this statement"
            )

        # TODO: replace global rules like: ssa_identifier, attr, etc with module constants: kirin.parse.grammar.SSA_IDENTIFIER, kirin.parse.grammar.ATTR, etc
        num_results = len(stmt_fields.results)

        stmt_body = f'"{stmt_type.dialect.name}.{stmt_type.name}" '
        return_match = ", ".join("ssa_identifier" for _ in range(num_results))
        type_match = ", ".join(' "!" attr' for _ in range(num_results))
        stmt_args_rule = ", ".join(
            f'"{arg.name}" "=" ssa_identifier' for arg in stmt_fields.args
        )
        attr_args_rule = ", ".join(
            f'"{name}" "=" {grammar.attr_rules[type(attr.type)]}'
            for name, attr in stmt_fields.attributes.items()
        )

        stmt_rule = f'{stmt_body} "(" {stmt_args_rule} ")"'

        if len(attr_args_rule) > 0:
            stmt_rule = f'{stmt_rule} "{{" {attr_args_rule} "}}"'

        if len(return_match) > 0:
            stmt_rule = f'"{return_match} "=" {stmt_rule} ":" {type_match}'

        return stmt_rule

    def lower(
        self, parser: LarkParser, state: LoweringState, stmt: Statement
    ) -> Result:
        pass
