from kirin import types, interp
from kirin.analysis import ForwardFrame, TypeInference
from kirin.dialects import func
from kirin.dialects.eltype import ElType
from kirin.analysis.typeinfer.widen import widen

from . import absint
from .stmts import For, IfElse
from ._dialect import dialect

# Precision/performance heuristic: allow structural widening before falling back
# to top. Correctness requires a verified invariant regardless of this budget.
_LOOP_WIDENING_BUDGET = 16


@dialect.register(key="typeinfer")
class TypeInfer(absint.Methods):
    @interp.impl(IfElse)
    def if_else_(
        self,
        interp_: TypeInference,
        frame: ForwardFrame[types.TypeAttribute],
        stmt: IfElse,
    ):
        frame.set(
            stmt.cond, frame.get(stmt.cond).meet(types.Bool)
        )  # set cond backwards
        return super().if_else(interp_, frame, stmt)

    @interp.impl(For)
    def for_loop(
        self,
        interp_: TypeInference,
        frame: ForwardFrame[types.TypeAttribute],
        stmt: For,
    ):
        loop_vars = frame.get_values(stmt.initializers)
        body_block = stmt.body.blocks[0]
        block_args = body_block.args

        eltype_stmt = ElType(stmt.iterable)
        eltype = interp_.frame_eval(frame, eltype_stmt)
        eltype_stmt.drop_all_references()

        if not isinstance(eltype, tuple):  # error
            return
        item = eltype[0]

        if isinstance(body_block.last_stmt, func.Return):
            frame.set_values(block_args, (item,) + loop_vars)
            frame.worklist.append(interp.Successor(body_block, item, *loop_vars))
            return  # if terminate is Return, there is no result

        candidate = loop_vars
        # Each fallback update promotes at least one changing component to top.
        # Allow one update per component and a final invariant check.
        max_iterations = _LOOP_WIDENING_BUDGET + len(candidate) + 1
        for iteration in range(max_iterations):
            # A fresh frame prevents earlier visits and intermediate types from
            # contaminating inference under the current loop invariant.
            with interp_.new_frame(stmt, has_parent_access=True) as body_frame:
                yielded = interp_.frame_call_region(
                    body_frame, stmt, stmt.body, item, *candidate
                )

            if not isinstance(yielded, tuple):
                frame.set_values(body_frame.entries.keys(), body_frame.entries.values())
                return yielded

            required = tuple(
                initial.join(value) for initial, value in zip(loop_vars, yielded)
            )
            if all(new.is_subseteq(old) for old, new in zip(candidate, required)):
                frame.set_values(body_frame.entries.keys(), body_frame.entries.values())
                frame.set_values(block_args, (item,) + candidate)
                return candidate

            # Structural growth can also produce unbounded unions or nesting.
            # Fall back to top for changing components, then verify the invariant
            # with another body evaluation rather than returning a partial result.
            candidate = tuple(
                (
                    widen(old, new)
                    if iteration < _LOOP_WIDENING_BUDGET
                    else old if new.is_subseteq(old) else types.Any
                )
                for old, new in zip(candidate, yielded)
            )

        raise interp.InterpreterError(
            f"scf.For type inference did not converge after {max_iterations} iterations"
        )
