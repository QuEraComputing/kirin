from typing import Any
from dataclasses import field, dataclass

from kirin import ir, types
from kirin.rewrite import Walk, Chain, Fixpoint
from kirin.passes.abc import Pass
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.passes.typeinfer import TypeInfer
from kirin.dialects.ilist.rewrite import (
    List2IList,
    AllToForLoop,
    AnyToForLoop,
    MapToForLoop,
    FoldToForLoop,
    ScanToForLoop,
    ConstList2IList,
    ForEachToForLoop,
)


class IListDesugar(Pass):
    """This pass desugars the Python list dialect
    to the immutable list dialect by rewriting all
    constant `list` type into `IList` type.
    """

    def unsafe_run(self, mt: ir.Method[..., Any]) -> RewriteResult:
        for arg in mt.args:
            _check_list(arg.type, arg.type)
        return Fixpoint(Walk(Chain(ConstList2IList(), List2IList()))).rewrite(mt.code)


@dataclass
class IListToLoop(Pass):
    """Lower IList combinators to indexed SCF loops within one method.

    Handles Map, Foldl, Foldr, Scan, ForEach, Any and All. The result retains
    callback calls, IList construction and concatenation, integer indexing,
    ranges, tuples, and arithmetic. Separate callee bodies are unchanged.

    Requires current inferred types. For methods not marked inferred, runs
    type inference first. Rules leave unsupported type representations unchanged;
    callers requiring complete normalization must check for residual combinators.

    The dialect group must include scf, func, ilist, py.constant, py.len,
    py.indexing, py.binop, py.boolop, and py.tuple.
    """

    typeinfer: TypeInfer = field(init=False)
    rule: RewriteRule = field(init=False)

    def __post_init__(self) -> None:
        self.typeinfer = TypeInfer(self.dialects, no_raise=self.no_raise)
        self.rule = Fixpoint(
            Walk(
                Chain(
                    MapToForLoop(),
                    FoldToForLoop(),
                    ScanToForLoop(),
                    ForEachToForLoop(),
                    AnyToForLoop(),
                    AllToForLoop(),
                )
            )
        )

    def unsafe_run(self, mt: ir.Method[..., Any]) -> RewriteResult:
        result = RewriteResult()
        if not mt.inferred:
            result = self.typeinfer.unsafe_run(mt)
        return self.rule.rewrite(mt.code).join(result)


def _check_list(total: types.TypeAttribute, type_: types.TypeAttribute):
    if isinstance(type_, types.Generic):
        _check_list(total, type_.body)
        for var in type_.vars:
            _check_list(total, var)
        if type_.vararg:
            _check_list(total, type_.vararg.typ)
    elif isinstance(type_, types.PyClass):
        if issubclass(type_.typ, list):
            raise TypeError(
                f"Invalid type {total} for this kernel, use IList instead of {type_}."
            )
    return
