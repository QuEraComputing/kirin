from lark import Tree, Token

from kirin.decl import fields
from kirin.exceptions import LarkLoweringError
from kirin.ir.nodes.stmt import Statement
from kirin.parse.grammar import Grammar, LarkLowerResult, LarkLoweringState
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

        results = 'stmt_return_args "=" ' if len(stmt_fields.results) > 0 else ""
        attrs = "stmt_attr_args" if len(stmt_fields.attrs) > 0 else ""

        return f'{results} "{stmt_type.dialect.name}.{stmt_type.name}" stmt_ssa_args {attrs}'

    def lower(
        self, state: LarkLoweringState, stmt_type: type[Statement], tree: Tree
    ) -> LarkLowerResult:
        results = []
        attrs = {}
        match tree.children:
            case [
                Tree() as results_tree,
                Token(),
                Token(),
                Tree() as ssa_args_tree,
                Tree() as attrs_tree,
            ]:
                results = state.visit(results_tree).expect(list)
                ssa_args = state.visit(ssa_args_tree).expect(dict)
                attrs = state.visit(attrs_tree).expect(dict)
            case [Tree() as results_tree, Token(), Token(), Tree() as ssa_args_tree]:
                results = state.visit(results_tree).expect(list)
                ssa_args = state.visit(ssa_args_tree).expect(dict)
            case [Token(), Tree() as ssa_args_tree, Tree() as attrs_tree]:
                ssa_args = state.visit(ssa_args_tree).expect(dict)
                attrs = state.visit(attrs_tree).expect(dict)
            case [Token(), Tree() as ssa_args_tree]:
                ssa_args = state.visit(ssa_args_tree).expect(dict)
            case _:
                raise ValueError(f"Unexpected tree shape: {tree}")

        stmt = state.append_stmt(stmt_type(**ssa_args, **attrs))
        state.current_frame.defs.update(
            {result: ssa for result, ssa in zip(results, stmt.results)}
        )

        return LarkLowerResult(stmt)
