# Ergonomics Review: Multi-Result Paths for SCF Operations

**Reviewer persona:** Ergonomics/DX (Physicist) -- DSL author controlling optical tweezer arrays
**Date:** 2026-03-22
**Scope:** Evaluate Path A (expand `Continuation::Yield`) vs Path B (tuple/struct packing) for multi-result support

---

## Path A: Expand `Continuation::Yield` to Carry Multiple Values

### Toy Scenario: Multi-Result `If`

Struct definition:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$if {condition} then {then_body} else {else_body} -> ({results:type})")]
#[kirin(builders, type = T)]
pub struct If<T: CompileTimeValue> {
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    results: Vec<ResultValue>,   // <-- derive macros reject this today
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
```

First friction point: the derive macros emit a compile error -- `"ResultValue field cannot be a Vec, consider implementing the builder manually"` -- because the builder template allocates positional SSA indices (`Result(stmt, 0)`, `Result(stmt, 1)`, ...) and a dynamic-length Vec breaks that scheme. So before I can even write my struct, someone has to fix `kirin-derive-toolkit/src/template/builder_template/helpers.rs`. I am blocked at step zero.

Assuming the derive infrastructure is extended, the `Yield` variant changes from `Yield(V)` to `Yield(SmallVec<[V; 1]>)`. The interpret impl becomes:

```rust
fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where ...
{
    let cond = interp.read(self.condition)?;
    let block = match cond.is_truthy() {
        Some(true) => self.then_body,
        Some(false) => self.else_body,
        None => return Ok(Continuation::Fork(smallvec![
            (self.then_body, smallvec![]),
            (self.else_body, smallvec![]),
        ])),
    };
    let stage = interp.active_stage_info::<L>();
    interp.bind_block_args(stage, block, &[])?;
    let control = interp.eval_block(stage, block)?;
    match control {
        Continuation::Yield(values) => {
            // friction: must iterate and zip with results
            if values.len() != self.results.len() {
                return Err(InterpreterError::ArityMismatch {
                    expected: self.results.len(),
                    got: values.len(),
                }.into());
            }
            for (result, value) in self.results.iter().zip(values) {
                interp.write(*result, value)?;
            }
            Ok(Continuation::Continue)
        }
        other => Ok(other),
    }
}
```

Friction points:

1. **Arity checking is my problem.** Every dialect author writing an SCF-like operation must manually check `values.len() == self.results.len()`. This is the kind of bug I would write once, not notice, and then spend two hours debugging when my tweezer calibration program silently drops a value.

2. **Yield construction changes everywhere.** The `Yield<T>` struct currently has `value: SSAValue`. It would need `values: Vec<SSAValue>`. Every existing `scf.yield %x` in every program becomes `scf.yield (%x)` or similar. The `Yield::interpret` impl changes from `Ok(Continuation::Yield(v))` to wrapping a single value in a SmallVec. Every existing test program breaks.

3. **`run_nested_calls` and `Return` are affected.** `run_nested_calls` in `stack/exec.rs` currently does `Continuation::Return(v) | Continuation::Yield(v) => Some(v.clone())`. If Yield becomes multi-valued, this `v.clone()` no longer works -- it needs to handle a collection. But Return stays single-valued (functions return one thing). So now Return and Yield have asymmetric shapes inside the same match arm. The `StackInterpreter::eval_block` and `AbstractInterpreter::eval_block` both need updates.

4. **Abstract interpreter's `eval_block` match changes.** In `abstract_interp/interp.rs`, the `eval_block` method currently just propagates continuations. If Yield changes shape, the Call-handling code that extracts `return_value()` from an `AnalysisResult` needs to understand multi-value yields too. This is framework internals I should never have to think about.

### Concept Budget: Path A

| Concept | Where learned | Complexity |
|---------|---------------|------------|
| `ResultValue` | Struct field, derive docs | Low |
| `Vec<ResultValue>` | New -- derive infra extension | Med |
| `Continuation::Yield(SmallVec<[V; 1]>)` | Interpreter prelude, changed from `Yield(V)` | Med |
| Arity checking in interpret impl | No helper exists, manual zip+check | Med |
| SmallVec construction for single yields | Wrapping single values: `smallvec![v]` | Low |
| `run_nested_calls` contract change | Framework internals (should be invisible, but leaks) | High |
| `Yield<T>` format string change | `$yield (%v1, %v2)` vs `$yield %v` | Low |

**Total new concepts: 4** (Vec<ResultValue>, SmallVec yield, arity checking, yield construction)

### Boilerplate Count

For a two-result `If`, the interpret impl is ~25 lines (vs ~14 for single result today). The arity check adds 5 lines. The zip-and-write loop adds 3 lines. The `Yield` interpret impl adds 1 line (SmallVec wrapping). Net: **+10 lines per SCF-like operation**.

But the real cost is the framework changes: `run_nested_calls`, both `eval_block` implementations, and the derive infrastructure. Those are not my lines, but they block me until someone does them.

---

## Path B: Keep `Yield(V)` Single-Valued, Use Tuple/Struct Packing

### Toy Scenario: Multi-Result `If`

The struct definition stays almost the same as today:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$if {condition} then {then_body} else {else_body} -> {result:type}")]
#[kirin(builders, type = T)]
pub struct If<T: CompileTimeValue> {
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    result: ResultValue,        // single result, holds the packed tuple
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
```

The interpreter value type (what I define in my DSL) carries the tuple:

```rust
#[derive(Clone, Debug)]
enum TweezerValue {
    Int(i64),
    Float(f64),
    TrapConfig(TrapConfig),
    Tuple(Vec<TweezerValue>),   // <-- packing container
}
```

The interpret impl is identical to today's single-result `If`:

```rust
fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where ...
{
    let cond = interp.read(self.condition)?;
    let block = match cond.is_truthy() {
        Some(true) => self.then_body,
        Some(false) => self.else_body,
        None => return Ok(Continuation::Fork(smallvec![
            (self.then_body, smallvec![]),
            (self.else_body, smallvec![]),
        ])),
    };
    let stage = interp.active_stage_info::<L>();
    interp.bind_block_args(stage, block, &[])?;
    let control = interp.eval_block(stage, block)?;
    match control {
        Continuation::Yield(value) => {
            interp.write(self.result, value)?;  // write the tuple as one value
            Ok(Continuation::Continue)
        }
        other => Ok(other),
    }
}
```

The multi-result work happens in the DSL program itself. The body block yields a tuple, and after the `if`, I unpack it:

```
%packed = if %cond then {
    // ... compute position and momentum ...
    yield make_tuple(%pos, %vel)
} else {
    yield make_tuple(%default_pos, %default_vel)
} -> TweezerTuple
%pos, %vel = unpack_tuple(%packed)
```

Or, if I define an `Unpack` operation in my dialect:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$unpack {input} -> {first:type}, {second:type}")]
#[kirin(builders, type = T)]
pub struct Unpack2<T: CompileTimeValue> {
    input: SSAValue,
    first: ResultValue,
    second: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
```

Friction point: the derive macros reject multiple `ResultValue` fields too. Actually, wait -- let me recheck. The derive rejects `Vec<ResultValue>` and `Option<ResultValue>`, but *multiple individual `ResultValue` fields* should work since each gets a positional index (`Result(stmt, 0)`, `Result(stmt, 1)`). This is how the existing `SSAKind::Result(Statement, usize)` works -- the second parameter is the result index. So `Unpack2` with two `ResultValue` fields should compile. That is an important distinction.

But the multi-result *unpack* still requires me to define an unpack operation, teach the interpreter how to destructure the tuple, and thread the tuple type through my type system. This is DSL-level work, not framework work.

### Concept Budget: Path B

| Concept | Where learned | Complexity |
|---------|---------------|------------|
| `ResultValue` | Struct field, derive docs (unchanged) | Low |
| `Continuation::Yield(V)` | Interpreter prelude (unchanged) | Low |
| Tuple/struct value variant | My own Value enum | Low |
| Pack operation (make_tuple) | My own dialect | Med |
| Unpack operation | My own dialect, multiple `ResultValue` fields | Med |
| Multiple `ResultValue` fields in one struct | Derive docs, positional SSA indexing | Low |

**Total new concepts: 2** (pack/unpack operations in my dialect)

### Boilerplate Count

The SCF `If` interpret impl is unchanged: 14 lines. I add a `Tuple(Vec<V>)` variant to my value enum: 1 line. I write a `MakeTuple` operation (~15 lines struct + ~8 lines interpret). I write an `Unpack2` operation (~15 lines struct + ~10 lines interpret). Net: **+49 lines in my dialect**, but **0 lines in framework code** and **0 changes to existing SCF**.

For the common case (zero or one result), there is zero overhead -- everything is identical to today.

---

## Edge Cases

### Zero results (void if)

**Path A:** `Yield(SmallVec<[V; 1]>)` with an empty SmallVec. The interpret impl checks `self.results.is_empty()` and skips the write loop. The `Yield<T>` struct needs `values: Vec<SSAValue>` that can be empty. The format string needs to handle `yield` with no arguments. This requires derive infrastructure for `Vec<SSAValue>` in a terminator position (currently not tested).

**Path B:** Orthogonal to multi-result. Void-if is tracked separately (implementation-notes.md #3). The existing plan uses `Option<ResultValue>` (blocked by derive macros) or a sentinel void type. Either way, `Yield(V)` stays unchanged -- a void-if either yields a unit value that gets discarded, or `result` is `None` and the yield value is ignored. This decouples void-if from multi-result entirely.

**Winner: Path B.** Void-if and multi-result are independent problems. Path A forces them into the same mechanism.

### One result (current state)

**Path A:** Every existing `Yield(v)` becomes `Yield(smallvec![v])`. Every match site that does `Continuation::Yield(v) =>` becomes `Continuation::Yield(values) =>` and must index `values[0]`. The `run_nested_calls` function, which is framework-internal, needs to destructure a SmallVec where it previously destructured a value. This is a breaking change to every single interpret impl and every consumer of Continuation.

**Path B:** No change. Everything stays as-is. The single-result case is the default path.

**Winner: Path B.** Zero overhead for the common case.

### Many results (5+ values)

**Path A:** Works natively -- `SmallVec<[V; 1]>` spills to heap for >1 elements. The arity check and zip loop scale linearly. But the text format for `yield` becomes unwieldy: `yield (%a, %b, %c, %d, %e)`.

**Path B:** The tuple packing approach scales fine for representation, but unpacking 5 values requires either a generic `UnpackN` operation (which needs a way to express N result values -- back to `Vec<ResultValue>`) or specialized `Unpack2`, `Unpack3`, etc. The DSL text gets verbose: `%packed = if ... -> TweezerTuple5; %a, %b, %c, %d, %e = unpack5(%packed)`. For 5+ results, the tuple approach becomes awkward.

**Winner: Path A**, but only for this edge case. In practice, 5+ results from a single SCF operation is rare. MLIR's `scf.for` with 5 loop-carried values is unusual. My tweezer DSL would likely use at most 2-3 (position + velocity, or trap config + phase + timing).

---

## Lifetime Complexity

**Path A:** No new lifetimes exposed to dialect authors. The `SmallVec<[V; 1]>` is owned. However, the trait bounds on `Interpretable` might need adjustments if the framework requires `V: IntoIterator` or similar. In practice, `V: Clone` is sufficient since SmallVec owns its elements.

**Path B:** No new lifetimes or trait bounds at all. The value type is entirely under dialect author control.

**Winner: Tie**, but Path B is marginally safer because it introduces zero new bounds.

---

## Error Experience

### Path A: Common Mistakes

**Mistake 1: Forgetting arity check in interpret impl.**
No compile error. At runtime: `interp.write` is called with a `ResultValue` from index 2 but the Yield only carried 1 value. Result: index-out-of-bounds panic or silently writing to wrong SSA slot. The compiler cannot help here because the arity relationship between `results: Vec<ResultValue>` and the Yield payload is dynamic.

**Mistake 2: Using `Vec<ResultValue>` in struct definition today.**
Compile error: `"ResultValue field cannot be a Vec, consider implementing the builder manually."` This is clear and actionable, but the "implement the builder manually" path is undocumented. A physicist would not know what a "builder" is in this context.

**Mistake 3: Wrapping single value in SmallVec incorrectly.**
If a dialect author writes `Continuation::Yield(value)` instead of `Continuation::Yield(smallvec![value])` after the change, the compiler produces a type mismatch: `expected SmallVec<[V; 1]>, found V`. This is clear but annoying -- every existing code example and tutorial is wrong after the change.

### Path B: Common Mistakes

**Mistake 1: Forgetting to unpack after an if-expression.**
The result is a packed tuple value bound to a single SSA slot. If I try to use it as a scalar, my value type's operations will fail at runtime with a type mismatch (e.g., "cannot add Tuple to Int"). The error message depends on my value type's impl, which I control. I can make it clear.

**Mistake 2: Pack/unpack arity mismatch.**
If `make_tuple` packs 3 values but `unpack2` expects 2, this is a runtime error inside my `Unpack2::interpret`. I can write a clear error message because the check is in my code.

**Winner: Path B.** Errors are in code I wrote and can make clear. Path A's errors are in framework code where a physicist has no leverage.

---

## Learning Curve

**Path A:** A new dialect author reading the interpreter prelude sees `Continuation::Yield(SmallVec<[V; 1]>)`. They must now understand: what is SmallVec? Why `[V; 1]`? What does the `1` mean? When does it heap-allocate? None of these questions matter for their DSL, but the type signature forces them to engage. The prelude's 7 symbols become conceptually heavier because `Yield` is no longer "just returns a value."

**Path B:** A new dialect author sees `Continuation::Yield(V)`. One value in, one value out. If they need multiple results later, they define a tuple variant in their value type -- a concept they already understand from Rust enums. The framework teaches them nothing new; their own domain knowledge (tuples, structs) is sufficient.

**Winner: Path B.** The framework stays out of the way. I learn multi-result patterns from Rust, not from kirin internals.

---

## Summary Comparison

| Dimension | Path A (Expand Yield) | Path B (Tuple Packing) |
|-----------|----------------------|----------------------|
| Framework changes required | Derive infra + Continuation + eval_block + run_nested_calls | None |
| Dialect author boilerplate (2-result If) | +10 lines (arity check, zip loop) | +49 lines (pack/unpack ops), but 0 in SCF |
| New concepts for dialect author | 4 | 2 |
| Breaking change | Yes, all existing Yield sites | No |
| Void-if interaction | Coupled (empty SmallVec) | Decoupled (orthogonal) |
| Single-result overhead | SmallVec wrapping everywhere | None |
| 5+ result ergonomics | Good (native) | Awkward (chain of unpacks) |
| Error clarity | Runtime arity mismatches in framework code | Runtime type mismatches in my code |
| Time to unblock dialect author | Weeks (derive infra + interpreter refactor) | Now (zero framework changes) |

**My recommendation as a dialect author:** Path B. I can start using it today. The pack/unpack boilerplate is annoying for the 5+ result case, but I will cross that bridge when I have a tweezer experiment that genuinely needs five simultaneous accumulators. For 0-2 results, which covers every experiment I am planning for the next year, Path B adds zero complexity to the framework and keeps all the complexity in code I own.

Path A is the "right" answer from a compiler engineering perspective -- it matches MLIR's model exactly. But I am not a compiler engineer. I need to ship a tweezer calibration experiment next month. Path B lets me do that without waiting for derive infrastructure work that is estimated at 2-3 days but historically takes longer when it touches positional SSA indexing.

If kirin eventually needs full MLIR parity for multi-result operations across many dialects, Path A becomes worth the investment. But that is a framework-level decision, not a dialect-author-level one. The framework team should make that call based on how many dialects actually need 3+ results, not based on theoretical elegance.
