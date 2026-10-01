from typing import Iterable, final
from dataclasses import field, dataclass

from kirin import ir, types, interp
from kirin.analysis.forward import ForwardExtra, ForwardFrame

from .lattice import Value, Result, Unknown, PartialTuple, PartialLambda


@dataclass
class Frame(ForwardFrame[Result]):
    should_be_pure: set[ir.Statement] = field(default_factory=set)
    """If any ir.MaybePure is actually pure."""
    frame_is_not_pure: bool = False
    """If we hit any non-pure statement."""


def _fact_id(fact: Result) -> int:
    """Get the ID of a fact for use in a cache key."""
    return id(fact.data) if type(fact) is Value else id(fact)


def _holds_closure(facts: Iterable[Result]) -> bool:
    """Whether `facts` contain a closure fact, possibly inside a partial tuple."""
    return any(
        type(fact) is PartialLambda
        or (type(fact) is PartialTuple and _holds_closure(fact.data))
        for fact in facts
    )


@dataclass(frozen=True)
class _CallSummary:
    """What an analyzed call returned and left in its frame."""

    arguments: tuple[Result, ...]
    """Kept alive so that the ids in the call's key cannot be reused."""
    result: Result
    entries: tuple[tuple[ir.SSAValue, Result], ...]
    should_be_pure: frozenset[ir.Statement]
    frame_is_not_pure: bool

    def frame(self, node: ir.Statement) -> Frame:
        return Frame(
            node,
            entries=dict(self.entries),
            should_be_pure=set(self.should_be_pure),
            frame_is_not_pure=self.frame_is_not_pure,
        )


@final
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

    With `cache_calls` set, within one run, a call with the same callee and the
    same argument facts as a call already analyzed reuses that call's result
    and frame instead of analyzing the callee again. A run starts whenever a
    frame is created without a caller; the IR may change between runs, so
    nothing is reused across them. Calls cut off at the depth limit, and calls
    whose frame holds a closure, are analyzed every time.
    """

    keys = ("constprop",)
    lattice = Result

    cache_calls: bool = field(default=False, kw_only=True)
    """Whether identical calls within a run reuse one analysis. Off unless asked
    for: a reused call skips the method tables that would have run for it, and
    hands out the same facts where the callee would have produced fresh ones."""

    _interp: interp.Interpreter = field(init=False)
    _call_cache: dict[tuple, _CallSummary] = field(
        default_factory=dict, init=False, repr=False
    )
    _cutoffs: int = field(default=0, init=False, repr=False)
    """Calls cut off at the depth limit so far; a call during which this grows
    is not reused."""

    def __post_init__(self) -> None:
        super().__post_init__()
        self._interp = interp.Interpreter(
            self.dialects,
            debug=self.debug,
            max_depth=self.max_depth,
            max_python_recursion_depth=self.max_python_recursion_depth,
        )

    def initialize(self):
        super().initialize()
        self._interp.initialize()
        return self

    def call(
        self, node: ir.Statement | ir.Method, *args: Result, **kwargs: Result
    ) -> tuple[Frame, Result]:
        # `ForwardExtra.call(self, ...)` rather than `super().call(...)`: CPython
        # gives a starred call to a bound method its own C frame, which would
        # lower the recursion depth that analyses survive.
        if not self.cache_calls or isinstance(node, ir.Method) or not self.state.depth:
            # A `Method` comes back as its code once its run is set up, and a
            # call without a caller is a whole run, whose frame its caller reads.
            return ForwardExtra.call(self, node, *args, **kwargs)

        key = (
            node,
            tuple(map(_fact_id, args)),
            tuple((name, _fact_id(value)) for name, value in kwargs.items()),
        )
        if (summary := self._call_cache.get(key)) is not None:
            return summary.frame(node), summary.result

        cutoffs = self._cutoffs
        frame, result = ForwardExtra.call(self, node, *args, **kwargs)
        # A cut-off result depends on where the call was made, and analyses
        # downstream tell closures apart by fact identity, so neither is shared.
        if self._cutoffs == cutoffs and not _holds_closure(
            (result, *frame.entries.values())
        ):
            self._call_cache[key] = _CallSummary(
                (*args, *kwargs.values()),
                result,
                tuple(frame.entries.items()),
                frozenset(frame.should_be_pure),
                frame.frame_is_not_pure,
            )
        return frame, result

    def recursion_limit_reached(self) -> Result:
        self._cutoffs += 1
        return super().recursion_limit_reached()

    def initialize_frame(
        self, node: ir.Statement, *, has_parent_access: bool = False
    ) -> Frame:
        state = getattr(self, "state", None)  # unset before `initialize`
        if state is None or not state.depth:
            # A frame without a caller starts a new run.
            self._call_cache.clear()
        return Frame(node, has_parent_access=has_parent_access)

    def method_self(self, method: ir.Method) -> Result:
        return Value(method)

    def frame_eval(
        self, frame: Frame, node: ir.Statement
    ) -> interp.StatementResult[Result]:
        method = self.lookup_registry(frame, node)
        if method is None:
            if node.has_trait(ir.ConstantLike):
                return self.try_eval_const_pure(frame, node, ())
            elif node.has_trait(ir.Pure):
                values = frame.get_values(node.args)
                if types.is_tuple_of(values, Value):
                    return self.try_eval_const_pure(frame, node, values)

            if not node.has_trait(ir.Pure):
                # not pure, and no implementation, let's say it's not pure
                frame.frame_is_not_pure = True
            return tuple(Unknown() for _ in node._results)

        ret = method(self, frame, node)
        if node.has_trait(ir.IsTerminator) or node.has_trait(ir.Pure):
            return ret
        elif not node.has_trait(ir.MaybePure):  # cannot be pure at all
            frame.frame_is_not_pure = True
        elif (
            node not in frame.should_be_pure
        ):  # implementation cannot decide if it's pure
            frame.frame_is_not_pure = True
        return ret

    def try_eval_const_pure(
        self,
        frame: Frame,
        stmt: ir.Statement,
        values: tuple[Value, ...],
    ) -> interp.StatementResult[Result]:
        _frame = self._interp.initialize_frame(frame.code)
        _frame.set_values(stmt.args, tuple(x.data for x in values))
        method = self._interp.lookup_registry(frame, stmt)
        if method is not None:
            value = method(self._interp, _frame, stmt)
        else:
            return tuple(Unknown() for _ in stmt.results)
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
