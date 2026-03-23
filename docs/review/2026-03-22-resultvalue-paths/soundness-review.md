# Soundness Review: Multi-Result Design Paths

**Reviewer**: Soundness Adversary
**Date**: 2026-03-22
**Scope**: `Continuation` enum, `run_nested_calls`, `eval_block`, `ValueStore::write`, `AnalysisResult`, SCF dialect interpret impls

---

## Path A: Expand `Continuation::Yield/Return` to Carry Multiple Values

### Invariant Inventory

| # | Invariant | Status | Enforcement |
|---|-----------|--------|-------------|
| A1 | Yield arity matches parent's result slot count | **New** | Caller's responsibility (not enforced) |
| A2 | Return arity matches Call's result slot count | **New** | Caller's responsibility (not enforced) |
| A3 | `run_nested_calls` pending_results stack stays in sync with call/return nesting | **Weakened** | Runtime-enforced always (`NoFrame` error) but partial-write corruption precedes it |
| A4 | Multi-write atomicity: N writes to N result slots all succeed or none take effect | **New** | Not enforced |
| A5 | Fork branch symmetry: all branches yield the same number of values | **New** | Not enforced |
| A6 | `AnalysisResult::return_value` join is well-defined for multi-valued returns | **Weakened** | Not enforced (currently single `Option<V>`) |
| A7 | `eval_block` always returns `Yield(v)` for StackInterpreter | **Changed** from trivially true (1:1) to requiring all terminators to agree on arity | Caller's responsibility |

### Attack Scenarios

**A1/A2 -- Arity Mismatch (P1: Silent Data Corruption)**

*Violated invariant*: Yield carries 3 values but parent If has 1 ResultValue slot.

*API sequence*: A dialect author writes an SCF-like op with one `result: ResultValue` field. The body block's terminator yields 3 values via `Continuation::Yield(vec![v1, v2, v3])`. The parent op iterates yield values and result slots in lockstep with `zip()`. Two values are silently dropped. If `zip_longest` or explicit length checking is used instead, the behavior depends entirely on which was chosen -- but **nothing in the type system prevents the mismatch**. The dialect author must manually get this right.

*Reachability*: Normal use. Any dialect author who changes a body block's yield arity without updating the parent op triggers this.

*Consequence*: Silent value loss (with `zip`) or panic (with explicit assert). Neither is a `Result` error.

**A3 -- `run_nested_calls` Pending Results Desync (P1: Silent Data Corruption)**

*Violated invariant*: `pending_results` stack assumes 1:1 Call-to-Return writeback.

*Current code*: `run_nested_calls` pushes one `ResultValue` per `Call`, then pops one per `Return`. Path A changes `Call { result }` to `Call { results: Vec<ResultValue> }` and `Return(V)` to `Return(Vec<V>)`.

*API sequence*: A function returns 2 values. `run_nested_calls` must now pop `Vec<ResultValue>` and write each value. But `pending_results` currently stores flat `ResultValue` entries. Refactoring it to `Vec<Vec<ResultValue>>` adds nesting. If a callee returns fewer values than the caller's `results` vec expects, the extra result slots get stale/unwritten values. A subsequent `read()` of those SSA slots returns whatever was previously in the frame's `FxHashMap` -- which could be a value from a different statement entirely.

*Reachability*: Normal use when a function signature changes but callers are not updated.

*Consequence*: Silent reads of stale SSA values. No error raised.

**A4 -- Partial Write (P1: Silent Data Corruption)**

*Violated invariant*: Writing N results is not atomic.

*API sequence*: Parent op writes results in a loop: `for (result, value) in results.iter().zip(values.iter()) { interp.write(result, value)?; }`. The 3rd write fails (e.g., frame stack is empty due to a bug). Results 0 and 1 are already written. The error propagates up, but the caller has no rollback mechanism. The frame now contains partially-updated state.

*Reachability*: Adversarial construction required -- `write` currently only fails if there is no frame, which is unlikely mid-execution. However, a custom `ValueStore` implementation could fail on any write.

*Consequence*: Partial state visible to subsequent error-recovery code or abstract interpreter that continues after errors.

**A5 -- Fork Branch Arity Mismatch (P1: Silent Data Corruption)**

*Violated invariant*: `propagate_control` in the abstract interpreter joins `Return(v)` / `Yield(v)` values across branches. Currently this is `v.join(other_v)` for single values. With multi-value returns, branch A might yield `[v1, v2]` while branch B yields `[v1]`. The join logic must handle mismatched lengths.

*API sequence*: An `scf.if` with `then_body` yielding 2 values and `else_body` yielding 1 value (mistyped IR). The abstract interpreter's `propagate_control` joins them element-wise. The shorter branch silently drops the join for the extra element, or panics on index-out-of-bounds.

*Reachability*: Malformed IR. Not reachable through well-formed dialect authors, but reachable through hand-constructed IR.

*Consequence*: Panic or silent value loss in abstract interpretation.

**A6 -- AnalysisResult Single Return Value (P2: Panic or Architectural Breakage)**

*Violated invariant*: `AnalysisResult` stores `return_value: Option<V>`, singular. Multi-valued returns require `Option<Vec<V>>` or a similar structure. All downstream consumers of `return_value()` (including `is_subseteq`, summary caching, the call handler in `eval_block`) assume a single value.

*API sequence*: Not an API misuse -- this is a **structural incompleteness**. Path A cannot be implemented without redesigning `AnalysisResult`, `SummaryCache`, and every call to `return_value()`.

*Reachability*: Guaranteed to surface during implementation.

*Consequence*: Compile errors during implementation, or if worked around with `V = Vec<InnerV>`, loss of pointwise lattice properties.

### Summary

Path A introduces **4 new caller-responsibility invariants (A1, A2, A5, A7)** and **weakens 2 existing invariants (A3, A6)** that were previously trivially satisfied. The arity-match invariant (A1/A2) is the most dangerous: it is not type-enforced, not runtime-checked, and reachable through normal dialect authoring. The `run_nested_calls` refactoring (A3) has a high risk of introducing subtle desync bugs during implementation. The `AnalysisResult` incompatibility (A6) blocks the abstract interpreter entirely until resolved.

**Total attack surface**: 4 distinct P1 scenarios reachable through the public API.

---

## Path B: Keep `Yield(V)` Single-Valued, Use Tuple/Struct Packing

### Invariant Inventory

| # | Invariant | Status | Enforcement |
|---|-----------|--------|-------------|
| B1 | Yield/Return carry exactly one value | **Preserved** | Type-enforced (enum variant shape unchanged) |
| B2 | `run_nested_calls` pending_results 1:1 correspondence | **Preserved** | Runtime-enforced always (`NoFrame` error) |
| B3 | `eval_block` returns `Yield(v)` | **Preserved** | Runtime-enforced (StackInterpreter always wraps in Yield) |
| B4 | Unpack produces exactly N values matching N ResultValue slots | **New** | Depends on mechanism |
| B5 | Unpack is total (no panics, no partial results) | **New** | Depends on mechanism |
| B6 | The packed V round-trips correctly through all existing V-generic code | **New** | Type-enforced (V: Clone) |
| B7 | `AnalysisResult::return_value` remains well-defined | **Preserved** | Still `Option<V>` -- the packed tuple is a single V |
| B8 | Fork branch symmetry | **Preserved** | Both branches yield one packed V; join is V::join |

### Attack Scenarios

**B4 -- Unpack Arity Mismatch (P1 or P3 depending on mechanism)**

*Violated invariant*: Unpack returns wrong number of values.

*Mechanism 1 -- Trait-based `Unpack<N>`*: If the trait has an associated const or type-level arity, the compiler enforces the count. **Type-enforced.** Attack surface: zero.

*Mechanism 2 -- Convention-based (dialect author writes manual unpack)*: The dialect author calls `unpack()` on the yielded V and destructures into results. If they unpack 2 values but have 3 ResultValue slots, one slot is never written.

*API sequence (Mechanism 2)*: Dialect author implements `Interpretable` for a multi-result op. Calls `eval_block`, receives `Yield(packed_v)`. Unpacks into `(a, b)` but has `results: [r0, r1, r2]`. Writes `r0 = a`, `r1 = b`, forgets `r2`. A later `read(r2)` returns `UnboundValue`.

*Reachability*: Normal use under Mechanism 2. Requires adversarial construction under Mechanism 1.

*Consequence*: `UnboundValue` error on read (detected, not silent). Under Mechanism 1, unreachable.

**B5 -- Unpack Panic (P2: Panic Through Public API)**

*Violated invariant*: Unpack implementation panics.

*API sequence*: A custom `V` type implements `Unpack` by indexing into an internal Vec. The Vec has 1 element but `Unpack<2>` is called. The indexing panics.

*Reachability*: Adversarial V implementation. Not reachable with well-behaved value types.

*Consequence*: Panic (not a Result error). Severity depends on whether panics are acceptable in the interpreter (currently they are not -- all errors go through `Result`).

**B6 -- Packed Value Interacts Incorrectly with Lattice Operations (P3: Adversarial Construction)**

*Violated invariant*: `V::join` on packed tuples must be pointwise. If V is a flat lattice type (e.g., `Interval`), packing `(Interval, Interval)` into a single V requires the V type to understand tuple structure for join/widen/narrow.

*API sequence*: User defines `V = TupleValue(Vec<Interval>)`. Implements `AbstractValue` with `join` that joins the Vecs element-wise. But if one branch packs 2 values and another branch (erroneously) packs 3, the join silently drops or panics.

*Reachability*: Requires the user to define a broken V type. The framework does not introduce the bug -- the user does. However, the framework provides no guardrails against it.

*Consequence*: Silent incorrect join or panic, depending on the V implementation.

### Summary

Path B preserves **all 5 existing invariants** (B1-B3, B7, B8) by keeping the `Continuation` enum shape unchanged. It introduces **3 new invariants** (B4, B5, B6), but the critical one (B4) can be made type-enforced with Mechanism 1 (trait with type-level arity). The `run_nested_calls` stack, `AnalysisResult`, and abstract interpreter propagation code require **zero changes**. The attack surface is confined to the unpack boundary.

**Total attack surface**: 0-1 P1 scenarios depending on mechanism choice. 1 P2. 1 P3.

---

## Comparison

| Dimension | Path A | Path B |
|-----------|--------|--------|
| Existing invariants preserved | 3 of 5 | 5 of 5 |
| New unchecked invariants | 4 (A1, A2, A5, A7) | 0-1 (B4, mechanism-dependent) |
| P1 scenarios (silent corruption) | 4 | 0-1 |
| P2 scenarios (panic via API) | 1 | 1 |
| P3 scenarios (adversarial) | 0 | 1 |
| `run_nested_calls` changes | Major refactor (stack type change) | None |
| `AnalysisResult` changes | Major refactor (single -> multi return) | None |
| `propagate_control` changes | Must handle multi-value join/narrow | None |
| Derive macro impact | All `#[derive(Interpretable)]` impls change | None |
| Dialect author burden | Must manually match arity everywhere | Must implement unpack at the V level once |

### Verdict

**Path B has a strictly smaller attack surface.** It preserves every existing invariant, confines new complexity to a single boundary (unpack), and that boundary can be made type-safe with a `trait Unpack<const N: usize>` or similar const-generic mechanism. Path A spreads arity-matching responsibility across every dialect author, every call site, and both interpreter implementations, with no type-level enforcement possible.

Path B is also **easier to make sound**: the only new invariant (B4) has a clear type-level solution. Path A's invariants (A1, A2, A5) are fundamentally runtime properties -- the number of values a block yields is determined by its terminator at execution time, not at compile time.

**Recommended path**: Path B, with a const-generic `Unpack` trait for the type-safe variant. Fall back to convention-based unpack only if const generics prove too restrictive for the V type hierarchy.
