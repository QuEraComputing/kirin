from dataclasses import field, dataclass

from kirin import ir, interp
from kirin.analysis.forward import ForwardExtra, ForwardFrame

from .lattice import Pure, Value, NotPure, Unknown, JointResult


@dataclass
class ExtraFrameInfo:
    frame_is_not_pure: bool = False


@dataclass
class Propagate(ForwardExtra[JointResult, ExtraFrameInfo]):
    """Forward dataflow analysis for constant propagation.

    This analysis is a forward dataflow analysis that propagates constant values
    through the program. It uses the `JointResult` lattice to track the constant
    values and purity of the values.

    The analysis is implemented as a forward dataflow analysis, where the
    `eval_stmt` method is overridden to handle the different types of statements
    in the IR. The analysis uses the `interp.Interpreter` to evaluate the
    statements and propagate the constant values.

    When a statement is registered under the "constprop" key in the method table,
    the analysis will call the method to evaluate the statement instead of using
    the interpreter. This allows for custom handling of statements.
    """

    keys = ["constprop"]
    lattice = JointResult

    _interp: interp.Interpreter = field(init=False)

    def __post_init__(self) -> None:
        super().__post_init__()
        self._interp = interp.Interpreter(
            self.dialects,
            fuel=self.fuel,
            debug=self.debug,
            max_depth=self.max_depth,
            max_python_recursion_depth=self.max_python_recursion_depth,
        )

    def initialize(self):
        super().initialize()
        self._interp.initialize()
        return self

    def _try_eval_const_pure(
        self,
        frame: ForwardFrame[JointResult, ExtraFrameInfo],
        stmt: ir.Statement,
        values: tuple[Value, ...],
    ) -> interp.StatementResult[JointResult]:
        try:
            _frame = self._interp.new_frame(frame.code)
            _frame.set_values(stmt.args, tuple(x.data for x in values))
            value = self._interp.eval_stmt(_frame, stmt)
            if isinstance(value, tuple):
                return tuple(JointResult(Value(each), Pure()) for each in value)
            elif isinstance(value, interp.ReturnValue):
                return interp.ReturnValue(JointResult(Value(value.result), Pure()))
            elif isinstance(value, interp.Successor):
                return interp.Successor(
                    value.block,
                    *tuple(
                        JointResult(Value(each), Pure()) for each in value.block_args
                    ),
                )
        except interp.InterpreterError:
            pass
        return (self.void,)

    def eval_stmt(
        self, frame: ForwardFrame[JointResult, ExtraFrameInfo], stmt: ir.Statement
    ) -> interp.StatementResult[JointResult]:
        if stmt.has_trait(ir.ConstantLike):
            return self._try_eval_const_pure(frame, stmt, ())
        elif stmt.has_trait(ir.Pure):
            values = tuple(x.const for x in frame.get_values(stmt.args))
            if ir.types.is_tuple_of(values, Value):
                return self._try_eval_const_pure(frame, stmt, values)

        method = self.lookup_registry(frame, stmt)
        if method is not None:
            ret = method(self, frame, stmt)
            self._set_frame_not_pure(ret)
            return ret
        elif stmt.has_trait(ir.Pure):
            # fallback to top for other statements
            return (JointResult(Unknown(), Pure()),)
        else:
            if frame.extra is None:
                frame.extra = ExtraFrameInfo(True)
            return (JointResult(Unknown(), NotPure()),)

    def _set_frame_not_pure(self, result: interp.StatementResult[JointResult]):
        frame = self.state.current_frame()
        if isinstance(result, tuple) and all(x.purity is Pure() for x in result):
            return

        if isinstance(result, interp.ReturnValue) and result.result.purity is Pure():
            return

        if isinstance(result, interp.Successor) and all(
            x.purity is Pure() for x in result.block_args
        ):
            return

        if frame.extra is None:
            frame.extra = ExtraFrameInfo(True)

    def run_method(
        self, method: ir.Method, args: tuple[JointResult, ...]
    ) -> JointResult:
        return self.run_callable(
            method.code, (JointResult(Value(method), NotPure()),) + args
        )

    def finalize(
        self,
        frame: ForwardFrame[JointResult, ExtraFrameInfo],
        results: JointResult,
    ) -> JointResult:
        results = super().finalize(frame, results)
        if frame.extra is not None and frame.extra.frame_is_not_pure:
            return JointResult(results.const, NotPure())
        return results
