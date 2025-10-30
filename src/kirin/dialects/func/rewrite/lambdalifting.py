from kirin import ir
from kirin.passes import TypeInfer
from kirin.dialects import py
from kirin.rewrite.abc import RewriteRule, RewriteResult

from ..stmts import Lambda, Function, GetField
from .._dialect import dialect


@dialect.canonicalize
class LambdaLifting(RewriteRule):
    """Lifts func.Lambda methods embedded in py.Constant into func.Function.
    - Trigger on py.Constant
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        from kirin.dialects import py

        if not isinstance(node, py.Constant):
            return RewriteResult(has_done_something=False)
        method = self._get_method_from_constant(node)
        if method is None:
            return RewriteResult(has_done_something=False)
        if not isinstance(method.code, Lambda):
            return RewriteResult(has_done_something=False)
        self._promote_lambda(method)

        rewrite_result = TypeInfer(dialects=method.dialects).unsafe_run(method)
        return RewriteResult(has_done_something=True).join(rewrite_result)

    def _get_method_from_constant(self, const_stmt: py.Constant) -> ir.Method | None:
        pyattr_data = const_stmt.value
        if isinstance(pyattr_data, ir.PyAttr) and isinstance(
            pyattr_data.data, ir.Method
        ):
            return pyattr_data.data
        return None

    def _get_field_index(self, getfield_stmt: GetField) -> int | None:
        fld = getfield_stmt.attributes.get("field")
        if fld:
            return getfield_stmt.field
        else:
            return None

    def _promote_lambda(self, method: ir.Method) -> None:
        new_method = method.similar()
        assert isinstance(
            new_method.code, Lambda
        ), "expected method.code to be func.Lambda before promotion"

        captured_fields = method.fields
        if captured_fields:
            for stmt in new_method.code.body.blocks[0].stmts:
                if not isinstance(stmt, GetField):
                    continue
                idx = self._get_field_index(stmt)
                if idx is None:
                    continue
                captured = new_method.fields[idx]
                from kirin.dialects import py

                const_stmt = py.Constant(captured)
                const_stmt.insert_before(stmt)
                if stmt.results and const_stmt.results:
                    stmt.results[0].replace_by(const_stmt.results[0])
                stmt.delete()
                new_method.code

        fn = Function(
            sym_name=new_method.code.sym_name,
            slots=new_method.code.slots,
            signature=new_method.code.signature,
            body=new_method.code.body,
        )
        method.code = fn
