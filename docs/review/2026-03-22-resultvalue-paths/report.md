# ResultValue Multi-Result Design Evaluation — 2026-03-22

**Scope:** Evaluate design paths for consistent `Vec<ResultValue>` / `Option<ResultValue>` support across the kirin framework.

**Reviewers:** Formalism (PL Theorist), Ergonomics/DX (Physicist), Compiler Engineer, Soundness Adversary

**Per-reviewer reports:** `docs/review/2026-03-22-resultvalue-paths/<role>-review.md`

---

## Executive Summary

All 4 reviewers initially recommended Path B (single-valued Yield with tuple packing). After user review, the decision was revised: **aggressive API breaking is acceptable**, and the project will adopt a **positional multi-value design** (evolved from Alt D / Path A) combined with a **kirin-unpack dialect** for DSL-level tuple operations.

### Final Design Decisions

| Component | Decision |
|-----------|----------|
| `Continuation::Yield` | `Yield(SmallVec<[V; 1]>)` — multi-value, positional |
| `Continuation::Return` | `Return(SmallVec<[V; 1]>)` — multi-value |
| `Continuation::Call` | `Call { results: SmallVec<[ResultValue; 1]> }` — multi-result |
| `scf.yield` | `values: Vec<SSAValue>` — multi-value terminator |
| `AnalysisResult` | `return_values: Option<SmallVec<[V; 1]>>` — pointwise join |
| Builder template | Support `Vec<ResultValue>`, `Option<ResultValue>`, `SmallVec<[ResultValue; N]>` |
| Unpack mechanism | New `kirin-unpack` dialect for DSL-level pack/unpack ops |
| Void-if | `Option<ResultValue>` in builder, `Yield(smallvec![])` for empty yield |
| Routing | Positional (interpret-time pairing by parent), same as MLIR |

### Key Design Distinction

**IR-level multi-result** (Continuation changes) and **language-level tuple** (kirin-unpack dialect) are different levels of abstraction:
- Multi-result = multiple dataflow edges from one operation (IR/compiler concept)
- Tuple = one value of a product type (language/type system concept)
- Both coexist. Dialect authors choose which to use.

---

## Findings & User Decisions

### F1. [P1] [Accepted] Derive builder must support Vec/Option/SmallVec ResultValue
**File:** `kirin-derive-toolkit/src/template/builder_template/helpers.rs:279-297, 422-441`
**User note:** Also support `SmallVec<[ResultValue; N]>` as a result field type.

### F2. [P1] [Accepted — DSL-level operations] Unpack mechanism
**Decision:** New `kirin-unpack` dialect providing MakeTuple/Unpack operations. Dialect authors who want multi-return compose this dialect. Framework provides common type/value impls for stack and abstract interpreters.

### F3. [P2] [Accepted] Option<ResultValue> for void-if
**File:** `kirin-scf/src/lib.rs:57`
**Open design question:** What does the body block yield for void-if? Options: (A) yield unit value, If ignores it; (B) bare yield with empty `Yield(smallvec![])`; (C) new terminator.

### F4. [P2] [Accepted — documented] Alternative D (pre-routed yield) for future reference
**Note:** User asked about MLIR's dataflow analysis approach. MLIR uses `RegionBranchOpInterface` (interface-driven, not continuation-driven). Documented as context.

### F5. [P2] [Accepted] Multi-accumulator For via multi-value yield
**File:** `kirin-scf/src/interpret_impl.rs:248-253`

### F6. [P3] [Accepted — noted] Path B → Path A migration (now moot — going with multi-value directly)

---

## Implementation Cascade

### Continuation Enum (control.rs)
```rust
pub enum Continuation<V, Ext = Infallible> {
    Continue,
    Jump(Block, Args<V>),
    Fork(SmallVec<[(Block, Args<V>); 2]>),
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Args<V>,
        results: SmallVec<[ResultValue; 1]>,  // was: result: ResultValue
    },
    Return(SmallVec<[V; 1]>),  // was: Return(V)
    Yield(SmallVec<[V; 1]>),   // was: Yield(V)
    Ext(Ext),
}
```

### Files That Must Change

| File/Module | Change | Est. LOC |
|-------------|--------|----------|
| `kirin-interpreter/src/control.rs` | Yield, Return, Call variant changes | ~15 |
| `kirin-interpreter/src/stack/exec.rs` | `run_nested_calls` returns `SmallVec<[V; 1]>`, multi-result writeback | ~30 |
| `kirin-interpreter/src/stack/frame.rs` | `eval_block` wraps in `Yield(smallvec![v])` | ~10 |
| `kirin-interpreter/src/stack/transition.rs` | Call/Return advance changes | ~10 |
| `kirin-interpreter/src/abstract_interp/` | AnalysisResult, propagate_control pointwise join | ~50 |
| `kirin-interpreter/src/call.rs` | SSACFGRegion blanket impls | ~10 |
| `kirin-derive-toolkit` builder_template | Lift Vec/Option/SmallVec rejection | ~50 |
| `kirin-scf/src/lib.rs` | Yield values field, If/For results fields | ~20 |
| `kirin-scf/src/interpret_impl.rs` | Multi-value yield/accumulator handling | ~25 |
| `kirin-function/src/call.rs` | Call results field | ~10 |
| `kirin-function/src/ret.rs` | Return values field | ~10 |
| All dialect interpret impls (7 crates) | `Yield(v)` → `Yield(smallvec![v])`, `Return(v)` → `Return(smallvec![v])` | ~40 |
| New: `kirin-unpack` crate | MakeTuple, Unpack ops + common impls | ~200 |
| Tests | All Continuation match sites | ~30 |
| **Total** | | **~510** |

---

## Soundness Risks (from Soundness Adversary review)

| Risk | Severity | Mitigation |
|------|----------|------------|
| Yield arity mismatch (values.len() != results.len()) | P1 | Runtime check + `ArityMismatch` error in parent interpret impl |
| Return arity mismatch (Call.results vs Return values) | P1 | Runtime check in `run_nested_calls`: `Return.values.len() == pending_results.last().len()` |
| Partial write (N writes, Kth fails) | P1 | Accept — same risk as today for block arg binding |
| Fork branch arity mismatch (abstract interp) | P1 | Pointwise join with length assertion |
| AnalysisResult structural change | P2 | Full update (accepted) |

### Arity Guardrail Design

The framework enforces arity at two points:
1. **Return ↔ Call**: `run_nested_calls` checks `return_values.len() == call_results.len()` before writeback. Mismatch → `InterpreterError::ArityMismatch`.
2. **Yield ↔ parent results**: Parent interpret impl checks `yield_values.len() == self.results.len()`. Framework can provide a helper: `check_yield_arity(values, results) -> Result<(), ArityMismatch>`.

**Key semantic rule**: `Return([v1, v2])` means the IR function genuinely returns two SSA values. If the user's language has tuple-return semantics, the correct IR is `Return([Value::Tuple(...)])` — one value of tuple type. Arity mismatch between Return and Call indicates a dialect implementation bug.

---

## Architectural Strengths Preserved

1. **Trait decomposition** — ValueStore / StageAccess / BlockEvaluator unchanged structurally
2. **HasResults iterator** — Already supports 0, 1, or N results
3. **Dialect composability** — L on method level unchanged
4. **eval_block contract** — Still returns Yield for block exits

## Strength Lost

1. **Continuation<V> parametricity** — V now appears inside SmallVec, adding container structure. Mitigated: SmallVec<[V; 1]> is isomorphic to V for the common case.
