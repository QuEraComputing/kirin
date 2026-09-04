from kirin import ir
from kirin.dialects import py, func
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.ilist.stmts import Map, ForEach

from .._dialect import dialect


@dialect.post_inference
class HoistConstant(RewriteRule):
    """Hoist ``py.Constant`` statements out of ``ilist`` closure bodies.

    Constants may live in any block of the lambda CFG. They are moved before
    the lambda and re-introduced as captures; the corresponding ``getfield`` is
    always placed at the start of the **entry** block so it dominates every use.

    This deliberately matches ``py.Constant`` rather than every pure,
    loop-invariant statement. General loop-invariant code motion additionally
    needs a guarantee that moving an operation before a possibly empty loop is
    safe; Kirin's ``Pure`` trait only guarantees absence of side effects.
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, (Map, ForEach)):
            return RewriteResult()

        lambda_stmt = node.fn.owner
        if (
            not isinstance(lambda_stmt, func.Lambda)
            or lambda_stmt.parent_block is None
            or not lambda_stmt.body.blocks
        ):
            return RewriteResult()

        entry = lambda_stmt.body.blocks[0]
        if not entry.args:
            return RewriteResult()

        constants = [
            stmt
            for block in lambda_stmt.body.blocks
            for stmt in block.stmts
            if isinstance(stmt, py.Constant)
        ]
        if not constants:
            return RewriteResult()

        body_self = entry.args[0]
        captures = list(lambda_stmt.captured)
        hoisted = False

        for constant in constants:
            if not constant.result.uses:
                # This constant is dead. It will be removed by DCE, so don't hoist it.
                continue
            constant.detach()
            constant.insert_before(lambda_stmt)

            field = len(captures)
            captures.append(constant.result)

            getfield = func.GetField(obj=body_self, field=field)
            getfield.result.type = constant.result.type
            getfield.result.name = constant.result.name

            assert entry.first_stmt is not None
            getfield.insert_before(entry.first_stmt)
            constant.result.replace_by(getfield.result)
            hoisted = True

        if not hoisted:
            return RewriteResult()

        body_region = lambda_stmt.body
        lambda_stmt.regions = []
        replacement = func.Lambda(
            captured=tuple(captures),
            sym_name=lambda_stmt.sym_name,
            slots=lambda_stmt.slots,
            signature=lambda_stmt.signature,
            body=body_region,
        )
        replacement.result.type = lambda_stmt.result.type
        replacement.result.hints = dict(lambda_stmt.result.hints)
        replacement.result.name = lambda_stmt.result.name
        replacement.source = lambda_stmt.source
        lambda_stmt.replace_by(replacement)

        return RewriteResult(has_done_something=True)
