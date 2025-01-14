from typing import Dict
from dataclasses import dataclass

from kirin import ir
from kirin.analysis import const
from kirin.dialects import py, fcf, func
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.ir.nodes.stmt import Statement


@dataclass
class InlineFcfMap(RewriteRule):
    cp_results: Dict[ir.SSAValue, const.JointResult]

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        match node:
            case fcf.Map():
                return self.rewrite_fcf_map(node)
            case _:
                return RewriteResult()

    def rewrite_fcf_map(self, node: fcf.Map) -> RewriteResult:
        # TODO make this more generic without the need for the constprop results
        tmp = self.cp_results.get(node.coll, None)

        if (tmp is None) or (not isinstance(tmp.const, const.Value)):
            return RewriteResult()

        coll = tmp.const.data

        # rewrite to directly inline:
        # get the method:
        tpl_elem = []
        curr = node
        for i in coll:
            new_c = py.Constant(value=i)
            newstmt = func.Call(callee=node.fn, inputs=(new_c.result,), kwargs=())

            new_c.insert_after(curr)
            newstmt.insert_after(new_c)
            tpl_elem.append(newstmt.result)
            curr = newstmt

        # assemble tuple:
        tpl = py.tuple.New(values=tuple(tpl_elem))
        tpl.insert_after(curr)
        node.result.replace_by(tpl.result)
        node.delete()

        return RewriteResult(has_done_something=True)
