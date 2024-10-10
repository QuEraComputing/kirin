import ast
import sys

from kirin.lowering.result import Result
from kirin.lowering.state import LoweringState

class FromPythonAST:
    @property
    def names(self) -> list[str]: ...
    def lower(self, state: LoweringState, node: ast.AST) -> Result: ...
    def unreachable(self, state: LoweringState, node: ast.AST) -> Result: ...
    def lower_Module(self, state: LoweringState, node: ast.Module) -> Result: ...
    def lower_Interactive(
        self, state: LoweringState, node: ast.Interactive
    ) -> Result: ...
    def lower_Expression(
        self, state: LoweringState, node: ast.Expression
    ) -> Result: ...
    def lower_FunctionDef(
        self, state: LoweringState, node: ast.FunctionDef
    ) -> Result: ...
    def lower_AsyncFunctionDef(
        self, state: LoweringState, node: ast.AsyncFunctionDef
    ) -> Result: ...
    def lower_ClassDef(self, state: LoweringState, node: ast.ClassDef) -> Result: ...
    def lower_Return(self, state: LoweringState, node: ast.Return) -> Result: ...
    def lower_Delete(self, state: LoweringState, node: ast.Delete) -> Result: ...
    def lower_Assign(self, state: LoweringState, node: ast.Assign) -> Result: ...
    def lower_AugAssign(self, state: LoweringState, node: ast.AugAssign) -> Result: ...
    def lower_AnnAssign(self, state: LoweringState, node: ast.AnnAssign) -> Result: ...
    def lower_For(self, state: LoweringState, node: ast.For) -> Result: ...
    def lower_AsyncFor(self, state: LoweringState, node: ast.AsyncFor) -> Result: ...
    def lower_While(self, state: LoweringState, node: ast.While) -> Result: ...
    def lower_If(self, state: LoweringState, node: ast.If) -> Result: ...
    def lower_With(self, state: LoweringState, node: ast.With) -> Result: ...
    def lower_AsyncWith(self, state: LoweringState, node: ast.AsyncWith) -> Result: ...
    def lower_Raise(self, state: LoweringState, node: ast.Raise) -> Result: ...
    def lower_Try(self, state: LoweringState, node: ast.Try) -> Result: ...
    def lower_Assert(self, state: LoweringState, node: ast.Assert) -> Result: ...
    def lower_Import(self, state: LoweringState, node: ast.Import) -> Result: ...
    def lower_ImportFrom(
        self, state: LoweringState, node: ast.ImportFrom
    ) -> Result: ...
    def lower_Global(self, state: LoweringState, node: ast.Global) -> Result: ...
    def lower_Nonlocal(self, state: LoweringState, node: ast.Nonlocal) -> Result: ...
    def lower_Expr(self, state: LoweringState, node: ast.Expr) -> Result: ...
    def lower_Pass(self, state: LoweringState, node: ast.Pass) -> Result: ...
    def lower_Break(self, state: LoweringState, node: ast.Break) -> Result: ...
    def lower_Continue(self, state: LoweringState, node: ast.Continue) -> Result: ...
    def lower_Slice(self, state: LoweringState, node: ast.Slice) -> Result: ...
    def lower_BoolOp(self, state: LoweringState, node: ast.BoolOp) -> Result: ...
    def lower_BinOp(self, state: LoweringState, node: ast.BinOp) -> Result: ...
    def lower_UnaryOp(self, state: LoweringState, node: ast.UnaryOp) -> Result: ...
    def lower_Lambda(self, state: LoweringState, node: ast.Lambda) -> Result: ...
    def lower_IfExp(self, state: LoweringState, node: ast.IfExp) -> Result: ...
    def lower_Dict(self, state: LoweringState, node: ast.Dict) -> Result: ...
    def lower_Set(self, state: LoweringState, node: ast.Set) -> Result: ...
    def lower_ListComp(self, state: LoweringState, node: ast.ListComp) -> Result: ...
    def lower_SetComp(self, state: LoweringState, node: ast.SetComp) -> Result: ...
    def lower_DictComp(self, state: LoweringState, node: ast.DictComp) -> Result: ...
    def lower_GeneratorExp(
        self, state: LoweringState, node: ast.GeneratorExp
    ) -> Result: ...
    def lower_Await(self, state: LoweringState, node: ast.Await) -> Result: ...
    def lower_Yield(self, state: LoweringState, node: ast.Yield) -> Result: ...
    def lower_YieldFrom(self, state: LoweringState, node: ast.YieldFrom) -> Result: ...
    def lower_Compare(self, state: LoweringState, node: ast.Compare) -> Result: ...
    def lower_Call(self, state: LoweringState, node: ast.Call) -> Result: ...
    def lower_FormattedValue(
        self, state: LoweringState, node: ast.FormattedValue
    ) -> Result: ...
    def lower_JoinedStr(self, state: LoweringState, node: ast.JoinedStr) -> Result: ...
    def lower_Constant(self, state: LoweringState, node: ast.Constant) -> Result: ...
    def lower_NamedExpr(self, state: LoweringState, node: ast.NamedExpr) -> Result: ...
    def lower_TypeIgnore(
        self, state: LoweringState, node: ast.TypeIgnore
    ) -> Result: ...
    def lower_Attribute(self, state: LoweringState, node: ast.Attribute) -> Result: ...
    def lower_Subscript(self, state: LoweringState, node: ast.Subscript) -> Result: ...
    def lower_Starred(self, state: LoweringState, node: ast.Starred) -> Result: ...
    def lower_Name(self, state: LoweringState, node: ast.Name) -> Result: ...
    def lower_List(self, state: LoweringState, node: ast.List) -> Result: ...
    def lower_Tuple(self, state: LoweringState, node: ast.Tuple) -> Result: ...
    def lower_Del(self, state: LoweringState, node: ast.Del) -> Result: ...
    def lower_Load(self, state: LoweringState, node: ast.Load) -> Result: ...
    def lower_Store(self, state: LoweringState, node: ast.Store) -> Result: ...
    def lower_And(self, state: LoweringState, node: ast.And) -> Result: ...
    def lower_Or(self, state: LoweringState, node: ast.Or) -> Result: ...
    def lower_Add(self, state: LoweringState, node: ast.Add) -> Result: ...
    def lower_BitAnd(self, state: LoweringState, node: ast.BitAnd) -> Result: ...
    def lower_BitOr(self, state: LoweringState, node: ast.BitOr) -> Result: ...
    def lower_BitXor(self, state: LoweringState, node: ast.BitXor) -> Result: ...
    def lower_Div(self, state: LoweringState, node: ast.Div) -> Result: ...
    def lower_FloorDiv(self, state: LoweringState, node: ast.FloorDiv) -> Result: ...
    def lower_LShift(self, state: LoweringState, node: ast.LShift) -> Result: ...
    def lower_Mod(self, state: LoweringState, node: ast.Mod) -> Result: ...
    def lower_Mult(self, state: LoweringState, node: ast.Mult) -> Result: ...
    def lower_MatMult(self, state: LoweringState, node: ast.MatMult) -> Result: ...
    def lower_Pow(self, state: LoweringState, node: ast.Pow) -> Result: ...
    def lower_RShift(self, state: LoweringState, node: ast.RShift) -> Result: ...
    def lower_Sub(self, state: LoweringState, node: ast.Sub) -> Result: ...
    def lower_Invert(self, state: LoweringState, node: ast.Invert) -> Result: ...
    def lower_Not(self, state: LoweringState, node: ast.Not) -> Result: ...
    def lower_UAdd(self, state: LoweringState, node: ast.UAdd) -> Result: ...
    def lower_USub(self, state: LoweringState, node: ast.USub) -> Result: ...
    def lower_Eq(self, state: LoweringState, node: ast.Eq) -> Result: ...
    def lower_Gt(self, state: LoweringState, node: ast.Gt) -> Result: ...
    def lower_GtE(self, state: LoweringState, node: ast.GtE) -> Result: ...
    def lower_In(self, state: LoweringState, node: ast.In) -> Result: ...
    def lower_Is(self, state: LoweringState, node: ast.Is) -> Result: ...
    def lower_IsNot(self, state: LoweringState, node: ast.IsNot) -> Result: ...
    def lower_Lt(self, state: LoweringState, node: ast.Lt) -> Result: ...
    def lower_LtE(self, state: LoweringState, node: ast.LtE) -> Result: ...
    def lower_NotEq(self, state: LoweringState, node: ast.NotEq) -> Result: ...
    def lower_NotIn(self, state: LoweringState, node: ast.NotIn) -> Result: ...
    def lower_comprehension(
        self, state: LoweringState, node: ast.comprehension
    ) -> Result: ...
    def lower_ExceptHandler(
        self, state: LoweringState, node: ast.ExceptHandler
    ) -> Result: ...
    def lower_arguments(self, state: LoweringState, node: ast.arguments) -> Result: ...
    def lower_arg(self, state: LoweringState, node: ast.arg) -> Result: ...
    def lower_keyword(self, state: LoweringState, node: ast.keyword) -> Result: ...
    def lower_alias(self, state: LoweringState, node: ast.alias) -> Result: ...
    def lower_withitem(self, state: LoweringState, node: ast.withitem) -> Result: ...
    if sys.version_info >= (3, 10):
        def lower_Match(self, state: LoweringState, node: ast.Match) -> Result: ...
        def lower_match_case(
            self, state: LoweringState, node: ast.match_case
        ) -> Result: ...
        def lower_MatchValue(
            self, state: LoweringState, node: ast.MatchValue
        ) -> Result: ...
        def lower_MatchSequence(
            self, state: LoweringState, node: ast.MatchSequence
        ) -> Result: ...
        def lower_MatchSingleton(
            self, state: LoweringState, node: ast.MatchSingleton
        ) -> Result: ...
        def lower_MatchStar(
            self, state: LoweringState, node: ast.MatchStar
        ) -> Result: ...
        def lower_MatchMapping(
            self, state: LoweringState, node: ast.MatchMapping
        ) -> Result: ...
        def lower_MatchClass(
            self, state: LoweringState, node: ast.MatchClass
        ) -> Result: ...
        def lower_MatchAs(self, state: LoweringState, node: ast.MatchAs) -> Result: ...
        def lower_MatchOr(self, state: LoweringState, node: ast.MatchOr) -> Result: ...

    if sys.version_info >= (3, 11):
        def lower_TryStar(self, state: LoweringState, node: ast.TryStar) -> Result: ...

    if sys.version_info >= (3, 12):
        def lower_TypeVar(self, state: LoweringState, node: ast.TypeVar) -> Result: ...
        def lower_ParamSpec(
            self, state: LoweringState, node: ast.ParamSpec
        ) -> Result: ...
        def lower_TypeVarTuple(
            self, state: LoweringState, node: ast.TypeVarTuple
        ) -> Result: ...
        def lower_TypeAlias(
            self, state: LoweringState, node: ast.TypeAlias
        ) -> Result: ...

class NoSpecialLowering(FromPythonAST):
    pass
