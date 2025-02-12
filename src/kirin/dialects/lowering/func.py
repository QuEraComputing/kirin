import ast

from kirin import ir, types, lowering
from kirin.dialects import cf, func
from kirin.exceptions import DialectLoweringError

dialect = ir.Dialect("lowering.func")


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Return(
        self, state: lowering.LoweringState, node: ast.Return
    ) -> lowering.Result:
        if node.value is None:
            stmt = func.Return(state.append_stmt(func.ConstantNone()).result)
            state.append_stmt(stmt)
        else:
            result = state.visit(node.value).expect_one()
            stmt = func.Return(result)
            state.append_stmt(stmt)
        return lowering.Result()

    def lower_FunctionDef(
        self, state: lowering.LoweringState, node: ast.FunctionDef
    ) -> lowering.Result:
        self.assert_simple_arguments(node.args)
        signature = func.Signature(
            inputs=tuple(
                self.get_hint(state, arg.annotation) for arg in node.args.args
            ),
            output=self.get_hint(state, node.returns),
        )
        frame = state.current_frame

        entries: dict[str, ir.SSAValue] = {}
        entr_block = ir.Block()
        fn_self = entr_block.args.append_from(
            types.Generic(
                ir.Method, types.Tuple.where(signature.inputs), signature.output
            ),
            node.name + "_self",
        )
        entries[node.name] = fn_self
        for arg, type in zip(node.args.args, signature.inputs):
            entries[arg.arg] = entr_block.args.append_from(type, arg.arg)

        def callback(frame: lowering.Frame, value: ir.SSAValue):
            first_stmt = entr_block.first_stmt
            stmt = func.GetField(obj=fn_self, field=len(frame.captures) - 1)
            if value.name:
                stmt.result.name = value.name
            stmt.result.type = value.type
            stmt.source = state.source
            if first_stmt:
                stmt.insert_before(first_stmt)
            else:
                entr_block.stmts.append(stmt)
            return stmt.result

        func_frame = state.push_frame(
            lowering.Frame.from_stmts(
                node.body,
                state,
                entr_block=entr_block,
                globals=frame.globals,
                capture_callback=callback,
            )
        )
        func_frame.defs.update(entries)
        state.exhaust()

        for block in func_frame.curr_region.blocks:
            if not block.last_stmt or not block.last_stmt.has_trait(ir.IsTerminator):
                block.stmts.append(
                    cf.Branch(arguments=(), successor=func_frame.next_block)
                )

        none_stmt = func.ConstantNone()
        rtrn_stmt = func.Return(none_stmt.result)
        func_frame.next_block.stmts.append(none_stmt)
        func_frame.next_block.stmts.append(rtrn_stmt)
        state.pop_frame()

        if state.current_frame.parent is None:  # toplevel function
            stmt = frame.append_stmt(
                func.Function(
                    sym_name=node.name,
                    signature=signature,
                    body=func_frame.curr_region,
                )
            )
            return lowering.Result(stmt)

        if node.decorator_list:
            raise DialectLoweringError(
                "decorators are not supported on nested functions"
            )

        # nested function, lookup unknown variables
        first_stmt = func_frame.curr_region.blocks[0].first_stmt
        if first_stmt is None:
            raise DialectLoweringError("empty function body")

        captured = [value for value in func_frame.captures.values()]
        lambda_stmt = func.Lambda(
            tuple(captured),
            sym_name=node.name,
            signature=signature,
            body=func_frame.curr_region,
        )
        lambda_stmt.result.name = node.name
        # NOTE: Python automatically assigns the lambda to the name
        frame.defs[node.name] = frame.append_stmt(lambda_stmt).result
        return lowering.Result(lambda_stmt)

    def assert_simple_arguments(self, node: ast.arguments) -> None:
        if node.kwonlyargs:
            raise DialectLoweringError("keyword-only arguments are not supported")

        if node.posonlyargs:
            raise DialectLoweringError("positional-only arguments are not supported")

    @staticmethod
    def get_hint(state: lowering.LoweringState, node: ast.expr | None):
        if node is None:
            return types.Any

        try:
            t = state.get_global(node).unwrap()
            return types.hint2type(t)
        except:  # noqa: E722
            raise DialectLoweringError(f"expect a type hint, got {ast.unparse(node)}")
