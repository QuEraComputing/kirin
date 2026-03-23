# Formalism Review: Multi-Result Support in Kirin Interpreter

**Reviewer**: Formalism (PL Theorist)
**Date**: 2026-03-22
**Subject**: `Continuation::Yield` / `Return` arity expansion vs. value-level product encoding

---

## Current Formal Structure

The existing `Continuation<V, Ext>` forms a free algebra over control transfer operations. The critical observation is that `V` appears uniformly across all variants: `Return(V)`, `Yield(V)`, `Call { result: ResultValue, .. }`, and `Jump(Block, Args<V>)`. This uniformity is not accidental -- it reflects a coherent denotational picture where each operation produces exactly one value of a single abstract type.

In CPS terms (Appel 1992), the current design corresponds to **single-value continuations**: every continuation expects exactly one value. The `eval_block` contract enforces this -- `StackInterpreter::eval_block` always returns `Continuation::Yield(v)` for a single `v`. The abstract interpreter's `propagate_control` joins Return/Yield values pointwise via `existing.join(v)` -- also single-valued.

---

## Path A: Multi-Value Continuations

### Change

```
Yield(V) -> Yield(SmallVec<[V; 1]>)
Return(V) -> Return(SmallVec<[V; 1]>)
Call { result: ResultValue } -> Call { results: Vec<ResultValue> }
```

### Denotational Interpretation

Path A models **n-ary continuations** (Danvy & Filinski 1992). Each continuation point expects a tuple of values, where the arity is determined dynamically at runtime. In denotational terms, the return type of a block shifts from `V` to `V*` (the free monoid over V).

This corresponds to the MLIR operational model where operations produce a list of results, and SCF yield carries an arbitrary number of values. It is the natural encoding when the "shape" of multi-results varies per operation instance.

### Compositionality

**Positive**: SCF composition becomes straightforward. `If::interpret` receives `Yield(vec![v1, v2])` and writes each element to a corresponding `ResultValue`. `For`'s loop-carried state generalizes naturally from `carried = vec![value]` to `carried = values`.

**Negative**: Every dialect's `interpret` method now pattern-matches on `Yield(values)` and must agree on the length convention. There is no static guarantee that `values.len()` matches the operation's result count. The arity invariant becomes a runtime contract -- a form of **dependent typing** that Rust's type system cannot enforce.

### Type-Level Properties

- **Parametricity of V is preserved**: `Continuation<V>` remains parametric in `V`. The change only affects the container around V, not V itself.
- **Arity preservation is lost**: `eval_block` returns `Yield(SmallVec<[V; 1]>)` with no static guarantee on length. The connection between an operation's declared result count and the yielded vector length is purely conventional.
- **Fork joins become structurally complex**: `propagate_control` currently joins a single V. With `Yield(Vec<V>)`, the join must be pointwise over vectors of potentially different lengths -- requiring a "pad with bottom" convention or runtime arity checks. This is the product lattice `L^n` (Cousot & Cousot 1977), but `n` is not statically known.

### Abstract Interpretation Implications

The abstract interpreter's `propagate_control` handles `Return(v) | Yield(v)` by joining into a single `return_value: Option<V>`. Under Path A, this becomes `return_values: Option<Vec<V>>`, requiring pointwise join: `(v1 join w1, v2 join w2, ...)`. The `AnalysisResult` type must store `Vec<V>` instead of `Option<V>` for the return value, and `is_subseteq` must check component-wise.

Widening becomes a pointwise operation over the vector components. This is mathematically well-defined (the product lattice L^n has componentwise widening), but the variable-length nature means the widening implementation must handle length mismatches -- a source of subtle bugs in abstract interpretation frameworks (Bourdoncle 1993).

### Downstream Bound Count

Dialect authors gain no new trait bounds -- `V: Clone` suffices. But every `Interpretable` impl that handles Yield/Return must destructure a vector, adding 2-5 lines of indexing/length-checking per use site.

---

## Path B: Value-Level Product Encoding

### Change

`Yield(V)` and `Return(V)` remain as-is. Multi-result operations pack results into a single V (e.g., a tuple or struct), then unpack after yield via a trait or convention.

### Denotational Interpretation

Path B models **single-value continuations with products in the value domain**. In Plotkin's call-by-value lambda calculus (Plotkin 1975), this is the standard encoding: multi-argument functions are Church-encoded via tupling, `f(a, b) = f(<a, b>)`. The continuation algebra stays first-order; the complexity shifts into the value algebra.

This aligns with the **A-normal form** tradition (Flanagan et al. 1993): every let-binding produces exactly one value. Multi-results are let-bound as `let t = op(args); let r1 = fst(t); let r2 = snd(t)`. The IR already works this way for single results -- Path B extends the pattern.

### Compositionality

**Positive**: The `Continuation` type, `eval_block`, `run_nested_calls`, `propagate_control`, and `AnalysisResult` all remain unchanged. Zero modification to the interpreter core. Dialects that only need single results (arith, bitwise, cmp, constant) are completely unaffected.

**Negative**: Requires an "unpack" mechanism. Either:
1. A trait `Unpack<V>` that SCF operations call after receiving the yielded value.
2. A convention where V is `Vec<V>` or similar, and SCF operations index into it.
3. The dialect author writes manual unpacking in their `Interpretable` impl.

Option (1) introduces a new trait bound on V that propagates to all SCF users. Option (2) forces a specific V representation. Option (3) is maximally flexible but shifts complexity to dialect authors.

### Type-Level Properties

- **Parametricity of V is preserved and strengthened**: The continuation algebra makes no assumption about V's structure. Whether V is a scalar, a tuple, or a lattice element is entirely the domain's concern.
- **Arity preservation is irrelevant**: Each continuation carries exactly one V. The "arity" question is pushed into the value domain where the type system can provide guarantees (e.g., `V = (V1, V2)` is statically two-valued).
- **Fork joins remain simple**: `propagate_control` joins a single V. If V is a product lattice, the join is still a single `v.join(w)` call -- the product structure is internal to the Lattice impl.

### Abstract Interpretation Implications

This is the cleanest path for abstract interpretation. The Cousot framework operates on a single lattice L. If multi-results are needed, the lattice becomes a product L1 x L2 x ... x Ln, but this is a standard construction: join, meet, widening, and narrowing all lift pointwise. Crucially, the abstract interpreter's infrastructure does not change -- the product structure is internal to the AbstractValue impl.

The `AnalysisResult<V>` type, `is_subseteq`, `propagate_control`, and `run_forward` all work without modification. The V stored per SSA slot is simply a richer lattice element when multi-results are needed.

### Downstream Bound Count

Zero additional bounds for dialects that do not use multi-results. For SCF with multi-results under option (1), one additional bound per `Interpretable` impl: `V: Unpack`. Under option (3), zero additional bounds but more manual code.

---

## Alternative Formalisms

### Alternative C: Indexed Result Families (Statically-Arity-Aware Continuations)

Change `Continuation` to carry an associated type for arity:

```rust
trait HasArity { type Results; }
enum Continuation<R: HasArity, Ext> {
    Yield(R::Results),
    Return(R::Results),
    ...
}
```

This is the **indexed family** approach (Dybjer 1994). Each operation declares its result type statically, and the continuation is parameterized by it.

**Formal property**: This provides the strongest static guarantees -- arity mismatches are compile-time errors. However, it fundamentally breaks the current design where `Continuation<V>` is uniform. `eval_block` can no longer return a single `Continuation<V>` because different statements produce different result types. The trait `Interpretable` would need a GAT for result types, reintroducing the E0275 cycle that was carefully eliminated by putting L on the method level.

**Verdict**: Principled but incompatible with kirin's trait architecture. The cost of restructuring is disproportionate. Not recommended.

### Alternative D: CPS with Multiple Continuations (Danvy-Filinski)

Instead of carrying multiple values in one continuation, pass multiple continuations:

```rust
Yield { values: Vec<(ResultValue, V)> }
```

Each (ResultValue, V) pair is a "write destination + value" tuple. The parent operation does not unpack a vector -- the yield already tells the interpreter exactly where each value goes.

**Formal property**: This is a hybrid of Paths A and B. The continuation still carries a list, but each element is pre-paired with its destination. This eliminates the "index the yielded vector against the result list" step that Path A requires.

**Practical impact**: `Yield { values: SmallVec<[(ResultValue, V); 1]> }` is backward-compatible via `Yield { values: smallvec![(result, v)] }` for single results. SCF operations receive a pre-routed list and call `interp.write(rv, val)` in a loop. No unpacking trait needed.

**Abstract interpretation**: Pointwise join over the `(ResultValue, V)` pairs keyed by ResultValue. Since ResultValue is an SSA identifier, the join merges by SSA slot -- which is exactly what the abstract interpreter already does for block arguments.

**Verdict**: The most practical extension of Path A. Trades one degree of abstraction (V is paired with its destination) for eliminating the arity-matching problem.

### Alternative E: Writer-Effect Style (Side-Effecting Multi-Write)

Multi-result operations call `interp.write(result_i, value_i)` directly (as they already do for single results), then yield a unit/sentinel:

```rust
// In interpret() for a multi-result op:
interp.write(self.result1, v1)?;
interp.write(self.result2, v2)?;
Ok(Continuation::Continue)
```

No change to `Continuation` at all. Multi-result operations are not continuations -- they are effectful statements that happen to produce multiple bindings.

**Formal property**: This is the **writer monad** encoding. Each operation's denotation is `State -> (State', Continuation)` where multi-result writes mutate the state and the continuation carries no values. The clean separation means `Yield` and `Return` remain exclusively for block-level and function-level boundaries.

**Practical impact**: This already works for non-SCF operations (arith, cmp, etc. already call `interp.write` and return `Continue`). The question is whether SCF body blocks can adopt the same pattern. Currently, `scf.yield` produces `Yield(v)` to communicate a value back to the parent `If`/`For`. Under this alternative, `scf.yield` would write directly to the parent's result slots. This requires the yield operation to know its parent's `ResultValue` identifiers -- breaking the separation between child block and parent operation.

**Verdict**: Works for non-SCF multi-result ops but does not generalize to SCF yield, which fundamentally needs to communicate values upward through `eval_block`.

---

## Comparison Table

| Criterion | Path A (Multi-Value Cont.) | Path B (Value-Level Product) | Alt C (Indexed) | Alt D (Pre-Routed Yield) | Alt E (Writer Effect) |
|---|---|---|---|---|---|
| **Core type changes** | Continuation, AnalysisResult, run_nested_calls, propagate_control | None | Continuation, Interpretable, eval_block, all dialect impls | Continuation (Yield variant only) | None |
| **Downstream bounds (per use site)** | 0 new bounds | 0-1 new bounds (Unpack trait) | N/A (requires GAT refactor) | 0 new bounds | 0 new bounds |
| **Lines changed (interpreter core)** | ~150-200 | 0 | ~500+ | ~80-100 | 0 |
| **Lines changed (dialect impls)** | ~30 per SCF op | ~20 per SCF op (unpack call) | All dialects rewritten | ~15 per SCF op | ~10 per multi-result op |
| **Static arity safety** | None (runtime) | Partial (value type) | Full (compile-time) | None (runtime) | N/A |
| **Abstract interp impact** | Pointwise Vec join, length-mismatch handling | None (product in V) | Full refactor | Keyed join by ResultValue | None |
| **Parametricity of V** | Preserved | Preserved | Broken (V indexed by arity) | Preserved | Preserved |
| **Fork/widening complexity** | Higher (Vec<V> join) | Unchanged | N/A | Moderate (Vec<(RV,V)> join) | Unchanged |
| **Backward compatibility** | Breaking | Non-breaking | Breaking | Breaking (Yield shape) | Non-breaking |
| **Compile-time impact** | Minimal | Minimal | Severe (GAT solver) | Minimal | Minimal |
| **Conceptual complexity** | Medium (continuation carries list) | Low (V is richer) | High (type-level arity) | Medium-Low (pre-routed list) | Low (but limited scope) |

---

## Formal Recommendation

From a PL-theoretic perspective, **Path B is the more principled design**. The argument rests on three pillars:

1. **Plotkin's Adequacy**: In call-by-value semantics, the denotation of a term is a single value. Multi-results are a syntactic convenience -- the semantic content is a product. Encoding the product in V (Path B) keeps the continuation algebra faithful to the denotational semantics. Encoding it in the continuation (Path A) conflates the control structure with the data structure.

2. **Cousot Compositionality**: The abstract interpretation framework is defined over a single lattice. Path B preserves this -- the lattice simply becomes a product lattice when needed. Path A requires the framework to reason about variable-length vectors of lattice elements, which complicates the widening/narrowing/fixpoint machinery without formal benefit.

3. **Information Hiding**: Path B keeps the multi-result mechanism internal to the value domain. Dialects that do not use multi-results see zero change. This is the parametricity argument -- the less the continuation type knows about the structure of V, the more reusable the framework is.

However, **Alternative D deserves consideration** as a pragmatic middle ground. It extends Path A in a way that eliminates arity-matching bugs (each yielded value is pre-paired with its destination) while keeping the continuation single-point-of-truth for data flow. If the project needs MLIR-style n-ary yields as a first-class concept rather than an encoding, Alternative D is more honest about the IR's operational semantics than Path B's encoding.

The choice ultimately depends on whether kirin views multi-results as **fundamental** (the IR has n-ary operations) or **derived** (the IR has single-result operations that can produce product values). The former favors Path A / Alt D. The latter favors Path B. Given that kirin "does not need to match MLIR," Path B is the recommendation.

---

## References

- Appel, A. W. (1992). *Compiling with Continuations*. Cambridge University Press.
- Cousot, P. & Cousot, R. (1977). "Abstract interpretation: a unified lattice model for static analysis of programs." *POPL*.
- Bourdoncle, F. (1993). "Efficient chaotic iteration strategies with widenings." *FMPA*.
- Danvy, O. & Filinski, A. (1992). "Representing control: a study of the CPS transformation." *MSCS*.
- Flanagan, C., Sabry, A., Duba, B. F., & Felleisen, M. (1993). "The essence of compiling with continuations." *PLDI*.
- Plotkin, G. D. (1975). "Call-by-name, call-by-value and the lambda-calculus." *TCS*.
- Dybjer, P. (1994). "Inductive families." *FACS*.
- Lattner, C. et al. (2020). "MLIR: A Compiler Infrastructure for the End of Moore's Law." *arXiv:2002.11054*.
