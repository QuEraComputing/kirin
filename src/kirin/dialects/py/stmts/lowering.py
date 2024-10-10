import ast

from kirin.dialects.py import data, types
from kirin.exceptions import DialectLoweringError
from kirin.ir import SSAValue
from kirin.lattice import Lattice
from kirin.lowering import FromPythonAST, LoweringState, Result

from . import _stmts as py
from .dialect import dialect


@dialect.register
class PythonLowering(FromPythonAST):

    def lower_Assign(self, state: LoweringState, node: ast.Assign) -> Result:
        results: Result = state.visit(node.value)
        assert len(node.targets) == len(
            results
        ), "number of targets and results do not match"

        current_frame = state.current_frame
        match node:
            case ast.Assign(
                targets=[ast.Name(lhs_name, ast.Store())], value=ast.Name(_, ast.Load())
            ):
                stmt = py.Alias(
                    value=results[0], target=data.PyAttr(lhs_name)
                )  # NOTE: this is guaranteed to be one result
                stmt.result.name = lhs_name
                current_frame.defs[lhs_name] = state.append_stmt(stmt).result
            case _:
                for target, value in zip(node.targets, results.values):
                    match target:
                        # NOTE: if the name exists new ssa value will be
                        # used in the future to shadow the old one
                        case ast.Name(name, ast.Store()):
                            value.name = name
                            current_frame.defs[name] = value
                        case ast.Subscript(obj, slice):
                            obj = state.visit(obj).expect_one()
                            slice = state.visit(slice).expect_one()
                            stmt = py.SetItem(obj=obj, index=slice, value=value)
                            state.append_stmt(stmt)
                        case _:
                            raise DialectLoweringError(f"unsupported target {target}")
        return Result()  # python assign does not have value

    def lower_Constant(self, state: LoweringState, node: ast.Constant) -> Result:
        return Result(state.append_stmt(py.Constant(node.value)))

    def lower_BinOp(self, state: LoweringState, node: ast.BinOp) -> Result:
        lhs = state.visit(node.left).expect_one()
        rhs = state.visit(node.right).expect_one()

        if op := getattr(py.binop, node.op.__class__.__name__, None):
            stmt = op(lhs=lhs, rhs=rhs)
        else:
            raise DialectLoweringError(f"unsupported binop {node.op}")
        return Result(state.append_stmt(stmt))

    def lower_BoolOp(self, state: LoweringState, node: ast.BoolOp) -> Result:
        lhs = state.visit(node.values[0]).expect_one()
        match node.op:
            case ast.And():
                boolop = py.And
            case ast.Or():
                boolop = py.Or
            case _:
                raise DialectLoweringError(f"unsupported boolop {node.op}")

        for value in node.values[1:]:
            lhs = state.append_stmt(
                boolop(lhs=lhs, rhs=state.visit(value).expect_one())
            ).result
        return Result(lhs)

    def lower_UnaryOp(self, state: LoweringState, node: ast.UnaryOp) -> Result:
        if op := getattr(py.unary, node.op.__class__.__name__, None):
            return Result(state.append_stmt(op(state.visit(node.operand).expect_one())))
        else:
            raise DialectLoweringError(f"unsupported unary operator {node.op}")

    def lower_Compare(self, state: LoweringState, node: ast.Compare) -> Result:
        # NOTE: a key difference here is we need to lower
        # the multi-argument comparison operators into binary operators
        # since low-level comparision operators are binary + we need a static
        # number of arguments in each instruction
        lhs = state.visit(node.left).expect_one()

        comparators = [
            state.visit(comparator).expect_one() for comparator in node.comparators
        ]

        cmp_results: list[SSAValue] = []
        for op, rhs in zip(node.ops, comparators):
            if op := getattr(py.cmp, op.__class__.__name__, None):
                stmt = op(lhs=lhs, rhs=rhs)
            else:
                raise DialectLoweringError(f"unsupported compare operator {op}")
            state.append_stmt(stmt)
            cmp_results.append(Result(stmt).expect_one())
            lhs = rhs

        if len(cmp_results) == 1:
            return Result(cmp_results)

        lhs = cmp_results[0]
        for op in cmp_results[1:]:
            stmt = py.And(lhs=lhs, rhs=op)
            state.append_stmt(stmt)
            lhs = stmt.result

        return Result(lhs)

    def lower_Tuple(self, state: LoweringState, node: ast.Tuple) -> Result:
        return Result(
            state.append_stmt(
                stmt=py.NewTuple(
                    tuple(state.visit(elem).expect_one() for elem in node.elts)
                )
            )
        )

    def lower_Subscript(self, state: LoweringState, node: ast.Subscript) -> Result:
        value = state.visit(node.value).expect_one()
        slice = state.visit(node.slice).expect_one()
        if isinstance(node.ctx, ast.Load):
            stmt = py.GetItem(obj=value, index=slice)
        else:
            raise DialectLoweringError(f"unsupported subscript context {node.ctx}")
        state.append_stmt(stmt)
        return Result(stmt)

    def lower_List(self, state: LoweringState, node: ast.List) -> Result:
        elts = tuple(state.visit(each).expect_one() for each in node.elts)

        if len(elts):
            typ: Lattice[types.PyType] = elts[0].type
            for each in elts:
                typ = typ.join(each.type)
        else:
            typ = types.Any

        stmt = py.NewList(typ, values=tuple(elts))  # type: ignore
        state.append_stmt(stmt)
        return Result(stmt)

    def lower_Attribute(self, state: LoweringState, node: ast.Attribute) -> Result:
        if not isinstance(node.ctx, ast.Load):
            raise DialectLoweringError(f"unsupported attribute context {node.ctx}")
        value = state.visit(node.value).expect_one()
        stmt = py.GetAttr(obj=value, attrname=node.attr)
        state.append_stmt(stmt)
        return Result(stmt)

    def lower_Expr(self, state: LoweringState, node: ast.Expr) -> Result:
        return state.visit(node.value)

    def lower_Name(self, state: LoweringState, node: ast.Name) -> Result:
        name = node.id
        if isinstance(node.ctx, ast.Load):
            value = state.current_frame.get(name)
            if value is None:
                raise DialectLoweringError(f"{name} is not defined")
            return Result(value)
        elif isinstance(node.ctx, ast.Store):
            raise DialectLoweringError("unhandled store operation")
        else:  # Del
            raise DialectLoweringError("unhandled del operation")

    def lower_Slice(self, state: LoweringState, node: ast.Slice) -> Result:
        def value_or_none(expr: ast.expr | None) -> SSAValue:
            if expr is not None:
                return state.visit(expr).expect_one()
            else:
                return state.append_stmt(py.Constant(None)).result

        lower = value_or_none(node.lower)
        upper = value_or_none(node.upper)
        step = value_or_none(node.step)
        return Result(state.append_stmt(py.Slice(start=lower, stop=upper, step=step)))
