"""Assignment dialect for Python.

This module contains the dialect for the Python assignment statement, including:

- Statements: `Alias`, `SetItem`.
- The lowering pass for the assignments.
- The concrete implementation of the assignment statements.

This dialects maps Python assignment syntax.
"""

import ast

from kirin import ir, types, interp, lowering
from kirin.decl import info, statement
from kirin.print import Printer

dialect = ir.Dialect("py.assign")

T = types.TypeVar("T")


@statement(dialect=dialect)
class Alias(ir.Statement):
    name = "alias"
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})
    value: ir.SSAValue = info.argument(T)
    target: ir.PyAttr[str] = info.attribute()
    result: ir.ResultValue = info.result(T)

    def print_impl(self, printer: Printer) -> None:
        printer.print_name(self)
        printer.plain_print(" ")
        with printer.rich(style="symbol"):
            printer.plain_print(self.target.data)

        with printer.rich(style="keyword"):
            printer.plain_print(" = ")

        printer.print(self.value)


@statement(dialect=dialect)
class SetItem(ir.Statement):
    name = "setitem"
    traits = frozenset({lowering.FromPythonCall()})
    obj: ir.SSAValue = info.argument(print=False)
    value: ir.SSAValue = info.argument(print=False)
    index: ir.SSAValue = info.argument(print=False)


@statement(dialect=dialect)
class SetAttribute(ir.Statement):
    name = "setattr"
    traits = frozenset({lowering.FromPythonCall()})
    obj: ir.SSAValue = info.argument(print=False)
    attr: str = info.attribute()
    value: ir.SSAValue = info.argument(print=False)


@statement(dialect=dialect)
class TypeAssert(ir.Statement):
    traits = frozenset({lowering.FromPythonCall()})
    got: ir.SSAValue = info.argument(print=False)
    expected: types.TypeAttribute = info.attribute()
    result: ir.ResultValue = info.result()

    def __init__(self, got: ir.SSAValue, *, expected: types.TypeAttribute):
        super().__init__(
            args=(got,),
            attributes={"expected": expected},
            result_types=(expected,),
            args_slice={"got": 0},
        )


@dialect.register
class Concrete(interp.MethodTable):

    @interp.impl(Alias)
    def alias(self, interp, frame: interp.Frame, stmt: Alias):
        return (frame.get(stmt.value),)

    @interp.impl(SetItem)
    def setindex(self, interp, frame: interp.Frame, stmt: SetItem):
        frame.get(stmt.obj)[frame.get(stmt.index)] = frame.get(stmt.value)

    @interp.impl(SetAttribute)
    def set_attribute(self, interp, frame: interp.Frame, stmt: SetAttribute):
        obj = frame.get(stmt.obj)
        value = frame.get(stmt.value)
        setattr(obj, stmt.attr, value)

    # NOTE: we don't do much runtime type checking here, object with generic
    # types will unlikely work here.
    # TODO: consider runtime type checking by boxing the value
    @interp.impl(TypeAssert)
    def type_assert(self, interp_, frame: interp.Frame, stmt: TypeAssert):
        got = frame.get(stmt.got)
        got_type = types.PyClass(type(got))
        if not got_type.is_subseteq(stmt.expected):
            raise TypeError(f"Expected {stmt.expected}, got {got_type}")
        return (frame.get(stmt.got),)


@dialect.register(key="typeinfer")
class TypeInfer(interp.MethodTable):
    @interp.impl(TypeAssert)
    def type_assert(
        self, interp_, frame: interp.Frame[types.TypeAttribute], stmt: TypeAssert
    ):
        got = frame.get(stmt.got)
        if got.is_subseteq(stmt.expected):
            return (got.meet(stmt.expected),)
        return (types.Bottom,)


@dialect.register
class Lowering(lowering.FromPythonAST):

    def lower_Assign(self, state: lowering.State, node: ast.Assign) -> lowering.Result:
        result = state.lower(node.value)
        current_frame = state.current_frame
        match node:
            case ast.Assign(
                targets=[ast.Name(lhs_name, ast.Store())], value=ast.Name(_, ast.Load())
            ):
                stmt = Alias(
                    value=result.data[0], target=ir.PyAttr(lhs_name)
                )  # NOTE: this is guaranteed to be one result
                stmt.result.name = lhs_name
                current_frame.defs[lhs_name] = current_frame.push(stmt).result
            case _:
                for target in node.targets:
                    self.assign_item(state, target, result)

    def lower_AnnAssign(
        self, state: lowering.State, node: ast.AnnAssign
    ) -> lowering.Result:
        type_hint = self.get_hint(state, node.annotation)
        value = state.lower(node.value).expect_one()
        stmt = state.current_frame.push(TypeAssert(got=value, expected=type_hint))
        self.assign_item_value(state, node.target, stmt.result)

    def lower_AugAssign(
        self, state: lowering.State, node: ast.AugAssign
    ) -> lowering.Result:
        match node.target:
            case ast.Name(name, ast.Store()):
                rhs = ast.Name(name, ast.Load())
            case ast.Attribute(obj, attr, ast.Store()):
                rhs = ast.Attribute(obj, attr, ast.Load())
            case ast.Subscript(obj, slice, ast.Store()):
                rhs = ast.Subscript(obj, slice, ast.Load())
            case _:
                raise lowering.BuildError(f"unsupported target {node.target}")
        self.assign_item_value(
            state,
            node.target,
            state.lower(ast.BinOp(rhs, node.op, node.value)).expect_one(),
        )

    def lower_NamedExpr(
        self, state: lowering.State, node: ast.NamedExpr
    ) -> lowering.Result:
        value = state.lower(node.value).expect_one()
        self.assign_item_value(state, node.target, value)
        return value

    @classmethod
    def assign_item_value(cls, state: lowering.State, target, value: ir.SSAValue):
        current_frame = state.current_frame
        match target:
            case ast.Name(name, ast.Store()):
                value.name = name
                current_frame.defs[name] = value
            case ast.Attribute(obj, attr, ast.Store()):
                obj = state.lower(obj).expect_one()
                stmt = SetAttribute(obj, value, attr=attr)
                current_frame.push(stmt)
            case ast.Subscript(obj, slice, ast.Store()):
                obj = state.lower(obj).expect_one()
                slice = state.lower(slice).expect_one()
                stmt = SetItem(obj=obj, index=slice, value=value)
                current_frame.push(stmt)
            case _:
                raise lowering.BuildError(f"unsupported target {target}")

    @classmethod
    def assign_item(cls, state: lowering.State, target, result: lowering.State.Result):
        match target:
            case ast.Tuple(elts, ast.Store()):
                if len(elts) != len(result.data):
                    raise lowering.BuildError(
                        f"tuple assignment length mismatch: {len(elts)} != {len(result.data)}"
                    )
                for target, value in zip(elts, result.data):
                    cls.assign_item_value(state, target, value)
            case _:
                cls.assign_item_value(state, target, result.expect_one())
