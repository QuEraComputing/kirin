"""Bounded constant-argument specialization of function and closure methods."""

from __future__ import annotations

import struct
from typing import Callable
from collections import deque
from dataclasses import dataclass

from kirin import ir, types
from kirin.rewrite import Walk, Chain, WrapConst, Call2Invoke
from kirin.analysis import const
from kirin.dialects import func
from kirin.passes.abc import Pass
from kirin.passes.fold import Fold
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.dialects.ilist import Map, New, Scan, Foldl, Foldr, IList, ForEach
from kirin.dialects.py.constant import Constant
from kirin.dialects.ilist.rewrite import Unroll, InlineGetItem
from kirin.rewrite.specialize_invoke import (
    SpecializeInvoke,
    SpecializationFactory,
    SpecializeClosureCall,
)


def _fact(value: ir.SSAValue) -> const.Result:
    if isinstance(value.owner, Constant):
        return const.Value(value.owner.value.unwrap())
    hint = value.hints.get("const")
    return hint if isinstance(hint, const.Result) else const.Unknown()


def constant_key(value: object) -> tuple | None:
    """Build a structural specialization key, or ``None`` if dynamic.
    The key is a tuple of the value's type and its contents, recursively.
    """
    if isinstance(value, ir.PyAttr):
        return constant_key(value.data)
    if isinstance(value, ir.Data):
        if type(value) is IList:
            items = tuple(constant_key(item) for item in value)
            if any(item is None for item in items):
                return None
            return (IList, id(value.elem), items)
        return (type(value), value)

    kind = type(value)
    if kind in (type(None), bool, int, str, bytes):
        return (kind, value)
    if type(value) is float:
        return (float, struct.pack("!d", value))
    if type(value) is complex:
        return (complex, struct.pack("!dd", value.real, value.imag))
    if type(value) is range:
        return (range, value.start, value.stop, value.step)
    if type(value) is tuple:
        items = tuple(constant_key(item) for item in value)
        if any(item is None for item in items):
            return None
        return (tuple, items)
    if type(value) is frozenset:
        items = tuple(constant_key(item) for item in value)
        if any(item is None for item in items):
            return None
        return (frozenset, frozenset(items))
    return None


@dataclass
class Specialize(Pass):
    """Specialize statically reachable func methods on supported constants.

    Each invocation starts from one method and owns a fresh cache.
    Generic methods retain their signatures; visited bodies may be folded.
    Known lambda-backed methods retain their bound closure fields. A call to a
    local lambda whose captures are still dynamic uses a clone of the lambda
    statement, created next to it with the same captures and the constant
    arguments bound; the clone's body is analyzed with the captures' facts.
    A closure shares its body with the lambda statement that created it, so
    it is cloned before that body is rewritten, even when no argument binds.
    Map, ForEach, Foldl, Foldr, and Scan operations with a statically known
    length are expanded to expose callback calls when at least one element
    is constant with a supported specialization key.
    A zero budget disables specialization.

    DSLs extend the bindable set by registering [`ir.Data`][kirin.ir.Data]
    attributes with structural ``__hash__`` / ``__eq__``. Builtin Python
    values in [`ir.PyAttr`][kirin.ir.PyAttr] stay on the careful whitelist.

    ``extra_rules`` supplies rewrite-rule factories for DSL-specific callsites.
    Each factory receives ``materialize`` and is instantiated once per pass run.
    Rules run after constant hinting alongside ``SpecializeInvoke``. They map
    operands to callee argument facts and retarget their own statements using
    the returned method and retained argument indices. Materialization shares
    this pass's cache, budget, and queue; created variants are processed normally.
    Custom callsites that are not rewritten are not traversed by this hook.
    """

    max_specializations: int = 32
    extra_rules: tuple[Callable[[SpecializationFactory], RewriteRule], ...] = ()

    def __post_init__(self):
        if self.max_specializations < 0:
            raise ValueError("max_specializations must be nonnegative")

    def unsafe_run(self, mt: ir.Method) -> RewriteResult:
        # Working state belongs to one invocation, even when the pass is reused.
        self.cache: dict[tuple, tuple[ir.Method, tuple[int, ...]]] = {}
        self.counts: dict[ir.Method, int] = {}
        self.generated: set[ir.Method] = set()
        self.pending: deque[ir.Method] = deque()
        self.seen: set[ir.Method] = set()
        self.names: set[str | None] = set()
        self.ordinal = 0
        self.closure_cache: dict[tuple, tuple[func.Lambda, tuple[int, ...]]] = {}
        self.closure_counts: dict[func.Lambda, int] = {}
        self.closures: list[func.Lambda] = []
        if self.max_specializations == 0:
            return RewriteResult()
        return self._run(mt)

    def _discover(self, root: ir.Method):
        # Reserve existing names without CallGraph's recursive descent. Also
        # inspect method constants, since Call2Invoke may resolve them later.
        pending = [root]
        seen: set[ir.Method] = set()
        while pending:
            mt = pending.pop()
            if mt in seen:
                continue
            seen.add(mt)
            self.names.add(mt.sym_name)
            if isinstance(mt.code, (func.Function, func.Lambda)):
                self.names.add(mt.code.sym_name)
            for stmt in mt.code.walk():
                if isinstance(stmt, func.Invoke):
                    pending.append(stmt.callee)
                elif (
                    isinstance(stmt, Constant)
                    and isinstance(stmt.value, ir.PyAttr)
                    and isinstance(stmt.value.data, ir.Method)
                ):
                    pending.append(stmt.value.data)

    def _analyze(self, mt: ir.Method) -> const.Frame:
        analysis = const.Propagate(mt.dialects)
        if self.no_raise:
            frame, _ = analysis.run_no_raise(mt)
        else:
            frame, _ = analysis.run(mt)
        return frame

    @staticmethod
    def _has_specializable_element(collection: ir.SSAValue) -> bool:
        def fact(value: ir.SSAValue):
            if isinstance(value.owner, Constant):
                return const.Value(value.owner.value.unwrap())
            return value.hints.get("const")

        known = fact(collection)
        if isinstance(known, const.Value) and isinstance(known.data, IList):
            return any(constant_key(element) is not None for element in known.data)
        if isinstance(collection.owner, New):
            # A partially static list need not have a constant collection fact.
            return any(
                isinstance(element := fact(value), const.Value)
                and constant_key(element.data) is not None
                for value in collection.owner.values
            )
        return False

    def _run(self, root: ir.Method) -> RewriteResult:
        self._discover(root)
        self.pending.append(root)
        result = RewriteResult()
        unroll = Unroll()
        specialize = Chain(
            SpecializeInvoke(self.materialize),
            SpecializeClosureCall(self.materialize_closure),
            *(factory(self.materialize) for factory in self.extra_rules),
        )
        rolled_ilist = (Map, ForEach, Foldl, Foldr, Scan)
        while self.pending:
            mt = self.pending.popleft()
            if mt in self.seen:
                continue
            self.seen.add(mt)
            if mt is not root and not self._owns_body(mt):
                continue
            old_callees = self._callees(mt)
            if any(isinstance(stmt, rolled_ilist) for stmt in mt.code.walk()):
                result = Fold(mt.dialects, no_raise=self.no_raise)(mt).join(result)
                for stmt in tuple(mt.code.walk()):
                    if not isinstance(stmt, rolled_ilist):
                        continue
                    if self._has_specializable_element(stmt.collection):
                        result = unroll.rewrite(stmt).join(result)
            frame = self._analyze(mt)
            result = Walk(WrapConst(frame)).rewrite(mt.code).join(result)
            for stmt in mt.code.walk():
                if isinstance(stmt, Constant) and isinstance(stmt.value, ir.PyAttr):
                    stmt.result.hints["const"] = const.Value(stmt.value.data)
            result = Walk(InlineGetItem()).rewrite(mt.code).join(result)
            result = Walk(Call2Invoke()).rewrite(mt.code).join(result)
            result = Walk(specialize).rewrite(mt.code).join(result)
            while self.closures:
                clone = self.closures.pop()
                result = self._specialize_closure(mt.dialects, clone, specialize).join(
                    result
                )
            result = Fold(mt.dialects, no_raise=self.no_raise)(mt).join(result)
            new_callees = self._callees(mt)
            for callee in set(old_callees) - set(new_callees):
                callee.backedges.discard(mt)
            mt.update_backedges()
            self.pending.extend(new_callees)
        return result

    @staticmethod
    def _owns_body(mt: ir.Method) -> bool:
        # A closure's code is the lambda statement that created it: still in a
        # block, or deleted by folding, which orphans the body. Only a detached
        # lambda, such as a clone from this pass, belongs to one method alone.
        code = mt.code
        return not isinstance(code, func.Lambda) or (
            code.parent is None and code.body.parent_node is code
        )

    @staticmethod
    def _callees(mt: ir.Method) -> tuple[ir.Method, ...]:
        return tuple(
            dict.fromkeys(
                stmt.callee for stmt in mt.code.walk() if isinstance(stmt, func.Invoke)
            )
        )

    def materialize(
        self, original: ir.Method, args: tuple[const.Result, ...]
    ) -> tuple[ir.Method, tuple[int, ...]] | None:
        """Reuse a specialization or create and schedule one within the budget."""
        if (
            not isinstance(original.code, (func.Function, func.Lambda))
            or original in self.generated
            or Constant.dialect not in original.dialects
        ):
            return None
        if len(args) != len(original.args):
            return None
        keys = tuple(
            constant_key(arg.data) if isinstance(arg, const.Value) else None
            for arg in args
        )
        if all(key is None for key in keys) and self._owns_body(original):
            return None
        key = (original, keys)
        if key in self.cache:
            return self.cache[key]
        count = self.counts.get(original, 0)
        if count >= self.max_specializations:
            return None
        original_entry = original.callable_region.blocks[0]
        if original_entry.first_stmt is None or any(
            original_entry in stmt.successors for stmt in original.code.walk()
        ):
            # Entry backedges would require adapting their argument lists too.
            return None
        retained = tuple(i for i, key in enumerate(keys) if key is None)
        clone = self._build_specialized_method(original, args, retained)
        self.counts[original] = count + 1
        self.generated.add(clone)
        self.cache[key] = (clone, retained)
        self.pending.append(clone)
        return clone, retained

    def _build_specialized_method(
        self,
        original: ir.Method,
        args: tuple[const.Result, ...],
        retained: tuple[int, ...],
    ) -> ir.Method:
        """Build and verify a clone with constants replacing omitted parameters."""
        clone = original.similar()
        code = clone.code
        assert isinstance(code, (func.Function, func.Lambda))
        if isinstance(code, func.Lambda):
            # This is an already-bound method: its fields hold the captures.
            # The detached clone must not reference the original creation site.
            code.captured = ()
        clone.sym_name = self._bind(code, args, retained, original.sym_name, original)
        clone.nargs = len(clone.callable_region.blocks[0].args)
        if original.arg_names is not None:
            clone.arg_names = [original.arg_names[0]] + [
                original.arg_names[i + 1] for i in retained
            ]
        clone.inferred = False
        clone.verify()
        return clone

    def materialize_closure(
        self, lam: func.Lambda, args: tuple[const.Result, ...]
    ) -> tuple[func.Lambda, tuple[int, ...]] | None:
        """Reuse or create a clone of a local closure with constant arguments.

        The clone is a lambda statement next to `lam` with the same captures,
        so a capture known only at run time stays a capture.
        """
        entry = lam.body.blocks[0]
        if lam.parent is None or not len(args) == len(lam.slots) == len(entry.args) - 1:
            return None
        keys = tuple(
            constant_key(arg.data) if isinstance(arg, const.Value) else None
            for arg in args
        )
        if all(key is None for key in keys):
            return None
        key = (lam, keys)
        if key in self.closure_cache:
            return self.closure_cache[key]
        count = self.closure_counts.get(lam, 0)
        if count >= self.max_specializations:
            return None
        if entry.first_stmt is None or any(
            entry in stmt.successors for stmt in lam.walk()
        ):
            # Entry backedges would require adapting their argument lists too.
            return None
        if any(not isinstance(use.stmt, func.GetField) for use in entry.args[0].uses):
            # The clone takes fewer arguments, so `self` may only read captures.
            return None
        retained = tuple(i for i, key in enumerate(keys) if key is None)
        clone = lam.from_stmt(lam, regions=[lam.body.clone()])
        self._bind(clone, args, retained, lam.sym_name)
        clone.insert_after(lam)
        self.closure_counts[lam] = count + 1
        self.closure_cache[key] = (clone, retained)
        self.closures.append(clone)
        return clone, retained

    def _specialize_closure(
        self, dialects: ir.DialectGroup, clone: func.Lambda, specialize: RewriteRule
    ) -> RewriteResult:
        """Hint and rewrite the body of a closure clone as for a method body.

        The enclosing method's analysis never enters a lambda body's frame, so
        the clone is analyzed on its own, with the facts of its captures.
        """
        captured = tuple(_fact(value) for value in clone.captured)
        inputs = tuple(const.Unknown() for _ in clone.signature.inputs)
        analysis = const.Propagate(dialects)
        try:
            with analysis.eval_context():
                frame, _ = analysis.call(
                    clone, const.PartialLambda(clone, captured), *inputs
                )
        except Exception:
            if not self.no_raise:
                raise
            return RewriteResult()
        body = clone.body
        result = Walk(WrapConst(frame)).rewrite(body)
        for stmt in body.walk():
            if isinstance(stmt, Constant) and isinstance(stmt.value, ir.PyAttr):
                stmt.result.hints["const"] = const.Value(stmt.value.data)
        result = Walk(InlineGetItem()).rewrite(body).join(result)
        result = Walk(Call2Invoke()).rewrite(body).join(result)
        return Walk(specialize).rewrite(body).join(result)

    def _bind(
        self,
        code: func.Function | func.Lambda,
        args: tuple[const.Result, ...],
        retained: tuple[int, ...],
        base: str,
        original: ir.Method | None = None,
    ) -> str:
        """Replace omitted parameters with constants and give `code` a new name.

        With `original`, `self` in the body is replaced by that method.
        """
        entry = code.body.blocks[0]
        first = entry.first_stmt
        assert first is not None
        # Hints belong to the analysis context, not the original syntax.
        for stmt in code.walk():
            for value in stmt.results:
                value.hints.pop("const", None)
        # Self inside the original body denotes the original method, including
        # recursive calls with its original arity and accesses to captures.
        if original is not None and entry.args[0].uses:
            method_const = Constant(original)
            method_const.insert_before(first)
            entry.args[0].replace_by(method_const.result)
        parameters = tuple(entry.args[1:])
        for i in reversed(range(len(args))):
            if i in retained:
                continue
            arg = args[i]
            assert isinstance(arg, const.Value)
            constant = Constant(arg.data)
            constant.result.name = parameters[i].name
            constant.insert_before(first)
            parameters[i].replace_by(constant.result)
            entry.args.delete(parameters[i])
        code.signature = func.Signature(
            tuple(code.signature.inputs[i] for i in retained), code.signature.output
        )
        code.slots = tuple(code.slots[i] for i in retained)
        entry.args[0].type = types.FunctionType(
            code.signature.inputs, code.signature.output
        )
        while True:
            self.ordinal += 1
            name = f"{base}_specialized_{self.ordinal}"
            if name not in self.names:
                break
        self.names.add(name)
        code.sym_name = name
        return name
