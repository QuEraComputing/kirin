from dataclasses import field, dataclass

from kirin import ir, types, interp
from kirin.analysis.forward import ForwardExtra, ForwardFrame

from .lattice import Value, Result, Unknown


@dataclass
class Frame(ForwardFrame[Result]):
    should_be_pure: set[ir.Statement] = field(default_factory=set)
    """If any ir.MaybePure is actually pure."""
    frame_is_not_pure: bool = False
    """If we hit any non-pure statement."""


@dataclass
class Propagate(ForwardExtra[Frame, Result]):
    """Forward dataflow analysis for constant propagation.

    This analysis is a forward dataflow analysis that propagates constant values
    through the program. It uses the `Result` lattice to track the constant
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
    lattice = Result

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

    def new_frame(self, code: ir.Statement) -> Frame:
        return Frame.from_func_like(code)

    def _try_eval_const_pure(
        self,
        frame: Frame,
        stmt: ir.Statement,
        values: tuple[Value, ...],
    ) -> interp.StatementResult[Result]:
        _frame = self._interp.new_frame(frame.code)
        _frame.set_values(stmt.args, tuple(x.data for x in values))
        method = self._interp.lookup_registry(frame, stmt)
        if method is not None:
            value = method(self._interp, _frame, stmt)
        else:
            return (Unknown(),)
        match value:
            case tuple():
                return tuple(Value(each) for each in value)
            case interp.ReturnValue(ret):
                return interp.ReturnValue(Value(ret))
            case interp.YieldValue(yields):
                return interp.YieldValue(tuple(Value(each) for each in yields))
            case interp.Successor(block, args):
                return interp.Successor(
                    block,
                    *tuple(Value(each) for each in args),
                )

    def eval_stmt(
        self, frame: Frame, stmt: ir.Statement
    ) -> interp.StatementResult[Result]:
        method = self.lookup_registry(frame, stmt)
        if method is None:
            if stmt.has_trait(ir.ConstantLike):
                return self._try_eval_const_pure(frame, stmt, ())
            elif stmt.has_trait(ir.Pure):
                values = frame.get_values(stmt.args)
                if types.is_tuple_of(values, Value):
                    return self._try_eval_const_pure(frame, stmt, values)

            if stmt.has_trait(ir.Pure):
                return (Unknown(),)  # no implementation but pure
            # not pure, and no implementation, let's say it's not pure
            frame.frame_is_not_pure = True
            return (Unknown(),)

        ret = method(self, frame, stmt)
        if stmt.has_trait(ir.IsTerminator) or stmt.has_trait(ir.Pure):
            return ret
        elif not stmt.has_trait(ir.MaybePure):  # cannot be pure at all
            frame.frame_is_not_pure = True
        elif (
            stmt not in frame.should_be_pure
        ):  # implementation cannot decide if it's pure
            frame.frame_is_not_pure = True
        return ret

    def run_method(
        self, method: ir.Method, args: tuple[Result, ...]
    ) -> tuple[Frame, Result]:
        return self.run_callable(method.code, (Value(method),) + args)
