import ast
import inspect

from kirin import ir
from kirin.dialects.func.attrs import Signature
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import Call, Function, GetField, Lambda, Return
from kirin.dialects.py import stmts, types
from kirin.exceptions import DialectLoweringError
from kirin.lowering import Frame, FromPythonAST, LoweringState, Result


@dialect.register
class FuncLowering(FromPythonAST):

    def lower_Call(self, state: LoweringState, node: ast.Call) -> Result:
        # 1. try to lookup global statement object
        # 2. lookup local values
        global_callee_result = state.get_global_nothrow(node.func)
        if global_callee_result is None:  # not found in globals, has to be local
            return_type = types.Any
            callee = state.visit(node.func).expect_one()
        else:
            global_callee = global_callee_result.unwrap()
            if inspect.isclass(global_callee):
                if issubclass(global_callee, ir.Statement):
                    if global_callee.dialect not in state.dialects.data:
                        raise DialectLoweringError(
                            f"unsupported dialect `{global_callee.dialect.name}`"  # type: ignore
                        )
                    return global_callee.from_python_call(state, node)
                elif issubclass(global_callee, slice):
                    return stmts.Slice.from_python_call(state, node)
                elif issubclass(global_callee, range):
                    return stmts.Range.from_python_call(state, node)

            # symbol exist in global, but not ir.Statement, lookup local first
            try:
                return_type = types.Any
                callee = state.visit(node.func).expect_one()
            except DialectLoweringError:  # not found in locals
                if isinstance(global_callee, ir.Method):  # global method
                    # return type is Any if not inferred yet
                    return_type = global_callee.return_type or types.Any
                    callee = state.append_stmt(stmts.Constant(global_callee)).result
                elif inspect.isfunction(global_callee) or inspect.isbuiltin(
                    global_callee
                ):
                    # TODO: allow custom lowering python builtin
                    raise DialectLoweringError(
                        f"unsupported callee: {type(global_callee)}, are you trying to call a python function?"
                    )
                else:
                    raise DialectLoweringError(
                        f"unsupported callee type: {type(global_callee)}"
                    )

        args = []
        for arg in node.args:
            if isinstance(arg, ast.Starred):  # TODO: support *args
                raise DialectLoweringError("starred arguments are not supported")
            else:
                args.append(state.visit(arg).expect_one())

        keywords = []
        for kw in node.keywords:
            keywords.append(kw.arg)
            args.append(state.visit(kw.value).expect_one())

        return Result(
            state.append_stmt(
                Call(callee, args, tuple(keywords), return_types=[return_type])
            )
        )

    def lower_Return(self, ctx: LoweringState, node: ast.Return) -> Result:
        if node.value is None:
            stmt = Return()
            ctx.append_stmt(stmt)
        else:
            result = ctx.visit(node.value).expect_one()
            stmt = Return(result)
            ctx.append_stmt(stmt)
        return Result(stmt)

    def lower_FunctionDef(self, state: LoweringState, node: ast.FunctionDef) -> Result:
        self.assert_simple_arguments(node.args)
        signature = Signature(
            inputs=tuple(
                FuncLowering.get_hint(state, arg.annotation) for arg in node.args.args
            ),
            output=FuncLowering.get_hint(state, node.returns),
        )
        frame = state.current_frame

        entries: dict[str, ir.SSAValue] = {}
        entry_block = ir.Block()
        fn_self = entry_block.args.append_from(
            types.PyGeneric(
                ir.Method, types.Tuple.where(signature.inputs), signature.output
            ),
            node.name + "_self",
        )
        entries[node.name] = fn_self
        for arg, type in zip(node.args.args, signature.inputs):
            entries[arg.arg] = entry_block.args.append_from(type, arg.arg)

        def callback(frame: Frame, value: ir.SSAValue):
            first_stmt = entry_block.first_stmt
            stmt = GetField(obj=fn_self, field=len(frame.captures) - 1)
            if value.name:
                stmt.result.name = value.name
            stmt.result.type = value.type
            if first_stmt:
                stmt.insert_before(first_stmt)
            else:
                entry_block.stmts.append(stmt)
            return stmt.result

        func_frame = state.push_frame(
            Frame.from_stmts(
                node.body,
                state,
                block=entry_block,
                globals=frame.globals,
                capture_callback=callback,
            )
        )
        func_frame.defs.update(entries)

        # NOTE: Python function returns None if no return statement
        # this is annoying, so we add a return statement at the end
        # so other control flows knows where to exit...
        # this can be a dead block if there is a return statement, but it's fine
        func_frame.next_block = ir.Block([Return()])
        state.exhaust(func_frame)
        if (
            func_frame.current_block.last_stmt
            and not func_frame.current_block.last_stmt.has_trait(ir.IsTerminator)
        ):
            func_frame.current_block.stmts.append(Return())

        func_frame.append_block(func_frame.next_block)
        state.pop_frame()
        # end of new frame

        if state.current_frame.parent is None:  # toplevel function
            stmt = frame.append_stmt(
                Function(
                    sym_name=node.name,
                    signature=signature,
                    body=func_frame.current_region,
                )
            )
            return Result(stmt)

        if node.decorator_list:
            raise DialectLoweringError(
                "decorators are not supported on nested functions"
            )

        # nested function, lookup unknown variables
        first_stmt = func_frame.current_region.blocks[0].first_stmt
        if first_stmt is None:
            raise DialectLoweringError("empty function body")

        captured = [value for value in func_frame.captures.values()]
        lambda_stmt = Lambda(node.name, signature, captured, func_frame.current_region)
        lambda_stmt.result.name = node.name
        # NOTE: Python automatically assigns the lambda to the name
        frame.defs[node.name] = frame.append_stmt(lambda_stmt).result
        return Result(lambda_stmt)

    def assert_simple_arguments(self, node: ast.arguments) -> None:
        if node.kwonlyargs:
            raise DialectLoweringError("keyword-only arguments are not supported")

        if node.posonlyargs:
            raise DialectLoweringError("positional-only arguments are not supported")

    @staticmethod
    def get_hint(ctx: LoweringState, node: ast.expr | None):
        if node is None:
            return types.Any

        try:
            t = ctx.get_global(node).unwrap()
            return types.hint2type(t)
        except:  # noqa: E722
            raise DialectLoweringError(f"expect a type hint, got {ast.unparse(node)}")
