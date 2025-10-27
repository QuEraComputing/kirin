from kirin import ir
from kirin.dialects import py, func
from kirin.rewrite.abc import RewriteRule, RewriteResult

from ._dialect import dialect


@dialect.canonicalize
class ClosureField(RewriteRule):
    """Lowers captured closure fields into py.Constants.
    - Trigger on func.Invoke
    - If the callee Method has non-empty .fields, lower its func.GetField to py.Constant
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, func.Invoke):
            return RewriteResult(has_done_something=False)
        method = node.callee
        if not method.fields:
            return RewriteResult(has_done_something=False)
        # Replace func.GetField with py.Constant.
        changed = self._lower_captured_fields(method)
        if changed:
            method.fields = ()
        from kirin.passes import TypeInfer

        rewrite_result = TypeInfer(dialects=method.dialects).unsafe_run(method)
        return RewriteResult(has_done_something=changed).join(rewrite_result)

    def _get_field_index(self, getfield_stmt: func.GetField) -> int | None:
        fld = getfield_stmt.attributes.get("field")
        if fld:
            return getfield_stmt.field
        else:
            return None

    def _lower_captured_fields(self, method: ir.Method) -> bool:
        changed = False
        fields = method.fields
        if not fields:
            return False

        for region in method.code.regions:
            for block in region.blocks:
                for stmt in list(block.stmts):
                    if not isinstance(stmt, func.GetField):
                        continue
                    idx = self._get_field_index(stmt)
                    if idx is None:
                        continue
                    captured = fields[idx]
                    # Skip Methods.
                    if isinstance(captured, ir.Method):
                        continue
                    # Replace GetField with Constant.
                    const_stmt = py.Constant(captured)
                    const_stmt.insert_before(stmt)
                    if stmt.results and const_stmt.results:
                        stmt.results[0].replace_by(const_stmt.results[0])
                    stmt.delete()
                    changed = True
        return changed
