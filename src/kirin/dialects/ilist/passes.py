from kirin import ir, types
from kirin.rewrite import Walk, Chain, Fixpoint
from kirin.passes.abc import Pass
from kirin.rewrite.result import RewriteResult
from kirin.dialects.ilist.rewrite import List2IList, ConstList2IList


class IListDesugar(Pass):
    """This pass desugars the Python list dialect
    to the immutable list dialect by rewriting all
    constant `list` type into `IList` type.
    """

    def unsafe_run(self, mt: ir.Method) -> RewriteResult:
        for arg in mt.args:
            _check_list(arg.type, arg.type)
        return Fixpoint(Walk(Chain(ConstList2IList(), List2IList()))).rewrite(mt.code)


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
