import ast

from kirin import ir
from kirin.dialects.func.attrs import Signature
from kirin.dialects.func.dialect import dialect
from kirin.dialects.func.stmts import (
    Call,
    ConstantNone,
    Function,
    GetField,
    Invoke,
    Lambda,
    Return,
)
from kirin.dialects.py import types
from kirin.exceptions import DialectLoweringError
from kirin.lowering import Frame, FromPythonAST, LoweringState, Result


@dialect.register
class FuncLowering(FromPythonAST):

    def lower_Call_local(
        self, state: LoweringState, callee: ir.SSAValue, node: ast.Call
    ) -> Result:
        args, keywords = self.__lower_Call_args_kwargs(state, node)
        stmt = Call(callee, args, kwargs=keywords)
        return Result(state.append_stmt(stmt))

    def lower_Call_global_method(
        self,
        state: LoweringState,
        method: ir.Method,
        node: ast.Call,
    ) -> Result:
        args, keywords = self.__lower_Call_args_kwargs(state, node)
        stmt = Invoke(args, callee=method, kwargs=keywords)
        stmt.result.type = method.return_type or types.Any
        return Result(state.append_stmt(stmt))

    def __lower_Call_args_kwargs(
        self,
        state: LoweringState,
        node: ast.Call,
    ):
        args: list[ir.SSAValue] = []
        for arg in node.args:
            if isinstance(arg, ast.Starred):  # TODO: support *args
                raise DialectLoweringError("starred arguments are not supported")
            else:
                args.append(state.visit(arg).expect_one())

        keywords = []
        for kw in node.keywords:
            keywords.append(kw.arg)
            args.append(state.visit(kw.value).expect_one())

        return tuple(args), tuple(keywords)

    def lower_Return(self, state: LoweringState, node: ast.Return) -> Result:
        if node.value is None:
            stmt = Return(state.append_stmt(ConstantNone()).result)
            state.append_stmt(stmt)
        else:
            result = state.visit(node.value).expect_one()
            stmt = Return(result)
            state.append_stmt(stmt)
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
            stmt.source = state.source
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
        none_const = ConstantNone()
        return_none = Return(none_const.result)
        none_const.source = state.source
        return_none.source = state.source
        func_frame.next_block = ir.Block([none_const, return_none])
        state.exhaust(func_frame)
        if (
            func_frame.current_block.last_stmt
            and not func_frame.current_block.last_stmt.has_trait(ir.IsTerminator)
        ):
            func_frame.append_stmt(
                Return(func_frame.append_stmt(ConstantNone()).result)
            )

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
        lambda_stmt = Lambda(
            tuple(captured),
            sym_name=node.name,
            signature=signature,
            body=func_frame.current_region,
        )
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
    def get_hint(state: LoweringState, node: ast.expr | None):
        if node is None:
            return types.Any

        try:
            t = state.get_global(node).unwrap()
            return types.hint2type(t)
        except:  # noqa: E722
            raise DialectLoweringError(f"expect a type hint, got {ast.unparse(node)}")
