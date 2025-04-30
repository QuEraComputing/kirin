import ast

from kirin import ir, types, lowering
from kirin.dialects import cf, func

dialect = ir.Dialect("lowering.func")


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Return(self, state: lowering.State, node: ast.Return) -> lowering.Result:
        if node.value is None:
            state.current_frame.push(
                func.Return(state.current_frame.push(func.ConstantNone()).result)
            )
        else:
            result = state.lower(node.value).expect_one()
            stmt = func.Return(result)
            state.current_frame.push(stmt)

    def lower_FunctionDef(
        self, state: lowering.State[ast.AST], node: ast.FunctionDef
    ) -> lowering.Result:
        self.assert_simple_arguments(node.args)
        signature = func.Signature(
            inputs=tuple(
                self.get_hint(state, arg.annotation) for arg in node.args.args
            ),
            output=self.get_hint(state, node.returns),
        )
        frame = state.current_frame

        slots = tuple(arg.arg for arg in node.args.args)
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

        with state.frame(
            node.body, entr_block=entr_block, capture_callback=callback
        ) as func_frame:
            func_frame.defs.update(entries)
            func_frame.exhaust()

            for block in func_frame.curr_region.blocks:
                if not block.last_stmt or not block.last_stmt.has_trait(
                    ir.IsTerminator
                ):
                    block.stmts.append(
                        cf.Branch(arguments=(), successor=func_frame.next_block)
                    )

            none_stmt = func.ConstantNone()
            rtrn_stmt = func.Return(none_stmt.result)
            func_frame.next_block.stmts.append(none_stmt)
            func_frame.next_block.stmts.append(rtrn_stmt)

        if state.current_frame.parent is None:  # toplevel function
            # TODO: allow a return value from function
            # and assign it to the function symbol
            frame.push(
                func.Function(
                    sym_name=node.name,
                    slots=slots,
                    signature=signature,
                    body=func_frame.curr_region,
                )
            )
            return

        if node.decorator_list:
            raise lowering.BuildError(
                "decorators are not supported on nested functions"
            )

        # nested function, lookup unknown variables
        first_stmt = func_frame.curr_region.blocks[0].first_stmt
        if first_stmt is None:
            raise lowering.BuildError("empty function body")

        lambda_stmt = func.Lambda(
            tuple(value for value in func_frame.captures.values()),
            sym_name=node.name,
            slots=slots,
            signature=signature,
            body=func_frame.curr_region,
        )
        lambda_stmt.result.name = node.name
        frame.push(lambda_stmt)
        # NOTE: Python automatically assigns the lambda to the name
        frame.defs[node.name] = lambda_stmt.result

    def assert_simple_arguments(self, node: ast.arguments) -> None:
        if node.kwonlyargs:
            raise lowering.BuildError("keyword-only arguments are not supported")

        if node.posonlyargs:
            raise lowering.BuildError("positional-only arguments are not supported")
