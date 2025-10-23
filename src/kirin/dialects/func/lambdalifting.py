from kirin import ir
from kirin.dialects import py, func
from kirin.rewrite.abc import RewriteRule, RewriteResult

from ._dialect import dialect


@dialect.canonicalize
class LambdaLifting(RewriteRule):
    """Lifts func.Lambda methods embedded in py.Constant into func.Function.
    - Trigger on py.Constant
    """

    def rewrite_Statement(self, node: ir.Statement) -> RewriteResult:
        if not isinstance(node, py.Constant):
            return RewriteResult(has_done_something=False)
        method = self._get_method_from_constant(node)
        if method is None:
            return RewriteResult(has_done_something=False)
        if not isinstance(method.code, func.Lambda):
            return RewriteResult(has_done_something=False)
        self._promote_lambda(method)
        return RewriteResult(has_done_something=True)

    def _get_method_from_constant(self, const_stmt: py.Constant) -> ir.Method | None:
        pyattr_data = const_stmt.value
        if isinstance(pyattr_data, ir.PyAttr) and isinstance(
            pyattr_data.data, ir.Method
        ):
            return pyattr_data.data
        return None

    def _promote_lambda(self, method: ir.Method) -> None:
        lambda_node = method.code
        assert isinstance(
            lambda_node, func.Lambda
        ), "expected method.code to be func.Function after promotion"
        fn = func.Function(
            sym_name=lambda_node.sym_name,
            slots=lambda_node.slots,
            signature=lambda_node.signature,
            body=lambda_node.body,
        )
        method.code = fn
        method.fields = tuple(lambda_node.captured)
