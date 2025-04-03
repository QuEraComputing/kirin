from __future__ import annotations

import ast
import sys
from abc import ABC
from typing import Any, Callable, ClassVar, TypeAlias
from dataclasses import dataclass

from kirin.ir import Method, SSAValue
from kirin.ir.attrs import types
from kirin.lowering.abc import Result
from kirin.lowering.state import State

LoweringTransform: TypeAlias = Callable[[Any, State[ast.AST], ast.Call], Result]

@dataclass
class Transform:
    objs: tuple[Callable, ...]
    func: LoweringTransform

@dataclass
class akin:
    obj: Callable

    def __call__(
        self,
        func: LoweringTransform | Transform,
    ) -> Transform: ...

class FromPythonAST(ABC):
    callee_table: ClassVar[dict[object, Transform]]

    @property
    def names(self) -> list[str]: ...
    def lower(self, state: State[ast.AST], node: ast.AST) -> Result: ...
    def unreachable(self, state: State[ast.AST], node: ast.AST) -> Result: ...
    def get_hint(
        self, state: State[ast.AST], node: ast.expr | None
    ) -> types.TypeAttribute: ...
    def lower_Module(self, state: State[ast.AST], node: ast.Module) -> Result: ...
    def lower_Interactive(
        self, state: State[ast.AST], node: ast.Interactive
    ) -> Result: ...
    def lower_Expression(
        self, state: State[ast.AST], node: ast.Expression
    ) -> Result: ...
    def lower_FunctionDef(
        self, state: State[ast.AST], node: ast.FunctionDef
    ) -> Result: ...
    def lower_AsyncFunctionDef(
        self, state: State[ast.AST], node: ast.AsyncFunctionDef
    ) -> Result: ...
    def lower_ClassDef(self, state: State[ast.AST], node: ast.ClassDef) -> Result: ...
    def lower_Return(self, state: State[ast.AST], node: ast.Return) -> Result: ...
    def lower_Delete(self, state: State[ast.AST], node: ast.Delete) -> Result: ...
    def lower_Assign(self, state: State[ast.AST], node: ast.Assign) -> Result: ...
    def lower_AugAssign(self, state: State[ast.AST], node: ast.AugAssign) -> Result: ...
    def lower_AnnAssign(self, state: State[ast.AST], node: ast.AnnAssign) -> Result: ...
    def lower_For(self, state: State[ast.AST], node: ast.For) -> Result: ...
    def lower_AsyncFor(self, state: State[ast.AST], node: ast.AsyncFor) -> Result: ...
    def lower_While(self, state: State[ast.AST], node: ast.While) -> Result: ...
    def lower_If(self, state: State[ast.AST], node: ast.If) -> Result: ...
    def lower_With(self, state: State[ast.AST], node: ast.With) -> Result: ...
    def lower_AsyncWith(self, state: State[ast.AST], node: ast.AsyncWith) -> Result: ...
    def lower_Raise(self, state: State[ast.AST], node: ast.Raise) -> Result: ...
    def lower_Try(self, state: State[ast.AST], node: ast.Try) -> Result: ...
    def lower_Assert(self, state: State[ast.AST], node: ast.Assert) -> Result: ...
    def lower_Import(self, state: State[ast.AST], node: ast.Import) -> Result: ...
    def lower_ImportFrom(
        self, state: State[ast.AST], node: ast.ImportFrom
    ) -> Result: ...
    def lower_Global(self, state: State[ast.AST], node: ast.Global) -> Result: ...
    def lower_Nonlocal(self, state: State[ast.AST], node: ast.Nonlocal) -> Result: ...
    def lower_Expr(self, state: State[ast.AST], node: ast.Expr) -> Result: ...
    def lower_Pass(self, state: State[ast.AST], node: ast.Pass) -> Result: ...
    def lower_Break(self, state: State[ast.AST], node: ast.Break) -> Result: ...
    def lower_Continue(self, state: State[ast.AST], node: ast.Continue) -> Result: ...
    def lower_Slice(self, state: State[ast.AST], node: ast.Slice) -> Result: ...
    def lower_BoolOp(self, state: State[ast.AST], node: ast.BoolOp) -> Result: ...
    def lower_BinOp(self, state: State[ast.AST], node: ast.BinOp) -> Result: ...
    def lower_UnaryOp(self, state: State[ast.AST], node: ast.UnaryOp) -> Result: ...
    def lower_Lambda(self, state: State[ast.AST], node: ast.Lambda) -> Result: ...
    def lower_IfExp(self, state: State[ast.AST], node: ast.IfExp) -> Result: ...
    def lower_Dict(self, state: State[ast.AST], node: ast.Dict) -> Result: ...
    def lower_Set(self, state: State[ast.AST], node: ast.Set) -> Result: ...
    def lower_ListComp(self, state: State[ast.AST], node: ast.ListComp) -> Result: ...
    def lower_SetComp(self, state: State[ast.AST], node: ast.SetComp) -> Result: ...
    def lower_DictComp(self, state: State[ast.AST], node: ast.DictComp) -> Result: ...
    def lower_GeneratorExp(
        self, state: State[ast.AST], node: ast.GeneratorExp
    ) -> Result: ...
    def lower_Await(self, state: State[ast.AST], node: ast.Await) -> Result: ...
    def lower_Yield(self, state: State[ast.AST], node: ast.Yield) -> Result: ...
    def lower_YieldFrom(self, state: State[ast.AST], node: ast.YieldFrom) -> Result: ...
    def lower_Compare(self, state: State[ast.AST], node: ast.Compare) -> Result: ...
    def lower_Call(self, state: State[ast.AST], node: ast.Call) -> Result: ...
    def lower_Call_global_method(
        self, state: State[ast.AST], method: Method, node: ast.Call
    ) -> Result: ...
    def lower_Call_local(
        self, state: State[ast.AST], callee: SSAValue, node: ast.Call
    ) -> Result: ...
    def lower_FormattedValue(
        self, state: State[ast.AST], node: ast.FormattedValue
    ) -> Result: ...
    def lower_JoinedStr(self, state: State[ast.AST], node: ast.JoinedStr) -> Result: ...
    def lower_Constant(self, state: State[ast.AST], node: ast.Constant) -> Result: ...
    def lower_NamedExpr(self, state: State[ast.AST], node: ast.NamedExpr) -> Result: ...
    def lower_TypeIgnore(
        self, state: State[ast.AST], node: ast.TypeIgnore
    ) -> Result: ...
    def lower_Attribute(self, state: State[ast.AST], node: ast.Attribute) -> Result: ...
    def lower_Subscript(self, state: State[ast.AST], node: ast.Subscript) -> Result: ...
    def lower_Starred(self, state: State[ast.AST], node: ast.Starred) -> Result: ...
    def lower_Name(self, state: State[ast.AST], node: ast.Name) -> Result: ...
    def lower_List(self, state: State[ast.AST], node: ast.List) -> Result: ...
    def lower_Tuple(self, state: State[ast.AST], node: ast.Tuple) -> Result: ...
    def lower_Del(self, state: State[ast.AST], node: ast.Del) -> Result: ...
    def lower_Load(self, state: State[ast.AST], node: ast.Load) -> Result: ...
    def lower_Store(self, state: State[ast.AST], node: ast.Store) -> Result: ...
    def lower_And(self, state: State[ast.AST], node: ast.And) -> Result: ...
    def lower_Or(self, state: State[ast.AST], node: ast.Or) -> Result: ...
    def lower_Add(self, state: State[ast.AST], node: ast.Add) -> Result: ...
    def lower_BitAnd(self, state: State[ast.AST], node: ast.BitAnd) -> Result: ...
    def lower_BitOr(self, state: State[ast.AST], node: ast.BitOr) -> Result: ...
    def lower_BitXor(self, state: State[ast.AST], node: ast.BitXor) -> Result: ...
    def lower_Div(self, state: State[ast.AST], node: ast.Div) -> Result: ...
    def lower_FloorDiv(self, state: State[ast.AST], node: ast.FloorDiv) -> Result: ...
    def lower_LShift(self, state: State[ast.AST], node: ast.LShift) -> Result: ...
    def lower_Mod(self, state: State[ast.AST], node: ast.Mod) -> Result: ...
    def lower_Mult(self, state: State[ast.AST], node: ast.Mult) -> Result: ...
    def lower_MatMult(self, state: State[ast.AST], node: ast.MatMult) -> Result: ...
    def lower_Pow(self, state: State[ast.AST], node: ast.Pow) -> Result: ...
    def lower_RShift(self, state: State[ast.AST], node: ast.RShift) -> Result: ...
    def lower_Sub(self, state: State[ast.AST], node: ast.Sub) -> Result: ...
    def lower_Invert(self, state: State[ast.AST], node: ast.Invert) -> Result: ...
    def lower_Not(self, state: State[ast.AST], node: ast.Not) -> Result: ...
    def lower_UAdd(self, state: State[ast.AST], node: ast.UAdd) -> Result: ...
    def lower_USub(self, state: State[ast.AST], node: ast.USub) -> Result: ...
    def lower_Eq(self, state: State[ast.AST], node: ast.Eq) -> Result: ...
    def lower_Gt(self, state: State[ast.AST], node: ast.Gt) -> Result: ...
    def lower_GtE(self, state: State[ast.AST], node: ast.GtE) -> Result: ...
    def lower_In(self, state: State[ast.AST], node: ast.In) -> Result: ...
    def lower_Is(self, state: State[ast.AST], node: ast.Is) -> Result: ...
    def lower_IsNot(self, state: State[ast.AST], node: ast.IsNot) -> Result: ...
    def lower_Lt(self, state: State[ast.AST], node: ast.Lt) -> Result: ...
    def lower_LtE(self, state: State[ast.AST], node: ast.LtE) -> Result: ...
    def lower_NotEq(self, state: State[ast.AST], node: ast.NotEq) -> Result: ...
    def lower_NotIn(self, state: State[ast.AST], node: ast.NotIn) -> Result: ...
    def lower_comprehension(
        self, state: State[ast.AST], node: ast.comprehension
    ) -> Result: ...
    def lower_ExceptHandler(
        self, state: State[ast.AST], node: ast.ExceptHandler
    ) -> Result: ...
    def lower_arguments(self, state: State[ast.AST], node: ast.arguments) -> Result: ...
    def lower_arg(self, state: State[ast.AST], node: ast.arg) -> Result: ...
    def lower_keyword(self, state: State[ast.AST], node: ast.keyword) -> Result: ...
    def lower_alias(self, state: State[ast.AST], node: ast.alias) -> Result: ...
    def lower_withitem(self, state: State[ast.AST], node: ast.withitem) -> Result: ...
    if sys.version_info >= (3, 10):
        def lower_Match(self, state: State[ast.AST], node: ast.Match) -> Result: ...
        def lower_match_case(
            self, state: State[ast.AST], node: ast.match_case
        ) -> Result: ...
        def lower_MatchValue(
            self, state: State[ast.AST], node: ast.MatchValue
        ) -> Result: ...
        def lower_MatchSequence(
            self, state: State[ast.AST], node: ast.MatchSequence
        ) -> Result: ...
        def lower_MatchSingleton(
            self, state: State[ast.AST], node: ast.MatchSingleton
        ) -> Result: ...
        def lower_MatchStar(
            self, state: State[ast.AST], node: ast.MatchStar
        ) -> Result: ...
        def lower_MatchMapping(
            self, state: State[ast.AST], node: ast.MatchMapping
        ) -> Result: ...
        def lower_MatchClass(
            self, state: State[ast.AST], node: ast.MatchClass
        ) -> Result: ...
        def lower_MatchAs(self, state: State[ast.AST], node: ast.MatchAs) -> Result: ...
        def lower_MatchOr(self, state: State[ast.AST], node: ast.MatchOr) -> Result: ...

    if sys.version_info >= (3, 11):
        def lower_TryStar(self, state: State[ast.AST], node: ast.TryStar) -> Result: ...

    if sys.version_info >= (3, 12):
        def lower_TypeVar(self, state: State[ast.AST], node: ast.TypeVar) -> Result: ...
        def lower_ParamSpec(
            self, state: State[ast.AST], node: ast.ParamSpec
        ) -> Result: ...
        def lower_TypeVarTuple(
            self, state: State[ast.AST], node: ast.TypeVarTuple
        ) -> Result: ...
        def lower_TypeAlias(
            self, state: State[ast.AST], node: ast.TypeAlias
        ) -> Result: ...

class NoSpecialLowering(FromPythonAST):
    pass
