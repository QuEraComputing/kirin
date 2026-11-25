"""Python half of the rewrite benchmark. Run with the checked-out Python's interpreter."""
from dataclasses import dataclass
import gc
import json
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import kirin
from kirin.dialects import func
from kirin.dialects.py.binop import Add, Sub
from kirin.dialects.py.constant import Constant
from kirin.dialects.py.unary import USub
from kirin.interp.concrete import Interpreter
from kirin.rewrite import Chain, DeadCodeElimination, Fixpoint, Walk
from kirin.rewrite.abc import RewriteResult, RewriteRule

from workload import workload

# Rust's `RewriteDriver::DEFAULT_MAX_SWEEPS`.
MAX_ITER = 10_000


@dataclass
class DoubleNeg(RewriteRule):
    """`-(-x)` is `x`; as in Rust, an unused match edits nothing and is no change."""

    def rewrite_Statement(self, node):
        if isinstance(node, USub) and isinstance(inner := node.value.owner, USub) and node.result.uses:
            node.result.replace_by(inner.value)
            return RewriteResult(has_done_something=True)
        return RewriteResult()


@dataclass
class UnrollSub(RewriteRule):
    """`v - k` with a constant `k > 1` is `(v - (k - 1)) - 1`, created before the match."""

    def rewrite_Statement(self, node):
        if not (isinstance(node, Sub) and node.result.uses and isinstance(c := node.rhs.owner, Constant)):
            return RewriteResult()
        k = c.value.unwrap()
        if not isinstance(k, int) or k <= 1:
            return RewriteResult()
        rest = Constant(k - 1)
        partial = Sub(node.lhs, rest.result)
        one = Constant(1)
        unrolled = Sub(partial.result, one.result)
        for stmt in (rest, partial, one, unrolled):
            stmt.insert_before(node)
        node.result.replace_by(unrolled.result)
        return RewriteResult(has_done_something=True)


@dataclass
class BackwardCascade(RewriteRule):
    match_cost: int

    def rewrite_Statement(self, node):
        current = node
        for _ in range(self.match_cost):
            if not isinstance(current, Add):
                break
            parent = current.lhs.owner
            if isinstance(parent, Sub) and parent.rhs is current.rhs and current.result.uses:
                current.result.replace_by(parent.lhs)
                return RewriteResult(has_done_something=True)
            current = parent
        if (isinstance(node, USub) and any(isinstance(use.stmt, func.Return) for use in node.result.uses)
                and isinstance(inner := node.value.owner, USub)):
            node.result.replace_by(inner.value)
            return RewriteResult(has_done_something=True)
        return RewriteResult()


def sweep(task, shape):
    """One iteration of the fixpoint: a walk of the rule, then a DCE walk."""
    if task == "dce":
        return Walk(DeadCodeElimination())
    if task == "backward_cascade":
        return Chain(Walk(BackwardCascade(int(shape.rsplit("_", 1)[1]))), Walk(DeadCodeElimination()))
    if task == "cascade":
        return Chain(Walk(UnrollSub()), Walk(DeadCodeElimination()))
    return Chain(Walk(DoubleNeg()), Walk(DeadCodeElimination()))


def primitive(op, method):
    block = method.callable_region.blocks[0]
    terminator = block.last_stmt
    statements = list(block.stmts)[:-1]
    for stmt in statements:
        if op == "bypass":
            stmt.result.replace_by(stmt.lhs)
            stmt.delete()
        elif op == "insert":
            Constant(0).insert_before(stmt)
        elif op == "move":
            stmt.detach()
            stmt.insert_before(terminator)
        elif op == "erase":
            stmt.delete()
        else:
            raise ValueError(op)
    return {"edits": len(statements)}


def apply(task, shape, method):
    if task in ("peephole", "no_match", "dce", "cascade", "backward_cascade"):
        result = Fixpoint(sweep(task, shape), max_iter=MAX_ITER).rewrite(method.code)
        return {"converged": not result.exceeded_max_iter}
    return primitive(task, method)


def checked(task, shape, method):
    """`apply`, counting sweeps the way Rust does: the last one changed nothing."""
    if task not in ("peephole", "no_match", "dce", "cascade", "backward_cascade"):
        return apply(task, shape, method)
    rule = sweep(task, shape)
    for sweeps in range(1, MAX_ITER + 1):
        if not rule.rewrite(method.code).has_done_something:
            return {"converged": True, "sweeps": sweeps}
    return {"converged": False, "sweeps": MAX_ITER}


def statements(method):
    return sum(len(list(block.stmts)) for block in method.callable_region.blocks)


def evaluate(method, samples):
    interpreter = Interpreter(method.dialects)
    return [interpreter.run(method, s["x"], s["y"], s["outer"], s["inner"])[1] for s in samples]


def measure(request):
    samples = []
    for _ in range(request["warmups"] + request["repeats"]):
        method = workload(request["shape"], request["size"])
        # Collect earlier IR between samples, not inside the timer.
        gc.collect()
        enabled = gc.isenabled()
        gc.disable()
        try:
            start = time.perf_counter()
            apply(request["task"], request["shape"], method)
            samples.append(time.perf_counter() - start)
        finally:
            if enabled:
                gc.enable()
        del method
    return samples[request["warmups"]:]


def main():
    request = json.load(sys.stdin)
    method = workload(request["shape"], request["size"])
    before = statements(method)
    summary = checked(request["task"], request["shape"], method)
    check = {"statements_before": before, "statements_after": statements(method),
             "values": evaluate(method, request["samples"]), "summary": summary}
    output = {"python": {"check": check, "timing": {"pass_seconds": measure(request)}}}
    print(json.dumps({"kirin_source": str(Path(kirin.__file__).resolve()), "variants": output}))


if __name__ == "__main__":
    main()
