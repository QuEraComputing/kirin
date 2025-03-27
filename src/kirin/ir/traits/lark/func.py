from typing import TYPE_CHECKING

import lark

from kirin.lowering import Frame
from kirin.parse.grammar import LarkLoweringState

from ..abc import LarkLoweringTrait

if TYPE_CHECKING:
    from kirin.dialects import func


class FunctionLowerTrait(LarkLoweringTrait["func.Function"]):

    def lark_rule(self, _, __):
        return '"func.func" IDENTIFIER signature region'

    def lower(
        self,
        state: LarkLoweringState,
        func_type: type["func.Function"],
        tree: lark.Tree,
    ):
        from kirin.dialects.func import Signature

        _, sym_name_tree, signature_tree, region_tree = tree.children

        sym_name = state.visit(sym_name_tree).expect(str)
        signature = state.visit(signature_tree).expect(Signature)
        state.push_frame(Frame.from_lark(state))

        state.visit(region_tree)

        return func_type(
            sym_name=sym_name,
            signature=signature,
            body=state.pop_frame(finalize_next=False),
        )
