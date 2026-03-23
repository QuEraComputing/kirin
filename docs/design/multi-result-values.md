# Multi-Result Values

This document specifies the design for consistent multi-result support across the
kirin framework: IR definitions, text format, parser, printer, interpreter, and
abstract interpreter.

## Motivation

Kirin operations can produce multiple SSA values (dataflow edges), but the framework
only partially supports this. The derive builder rejects `Vec<ResultValue>` and
`Option<ResultValue>` fields. The `Continuation::Yield` and `Continuation::Return`
variants carry a single value. SCF operations like `If` and `For` are limited to
one result, and `For` supports only a single loop-carried accumulator.

This design removes those limitations with a breaking API change across the
interpreter framework, derive infrastructure, and text format DSL.

## Key Distinction: IR Multi-Result vs Language-Level Tuple

These are **different levels of abstraction** and must not be conflated:

**IR-level multi-result** — an operation produces N separate SSA values, each with
its own type and independent downstream uses. This is a dataflow concept.

```
%r1, %r2 = call @foo(%x) -> i32, f64
// %r1 and %r2 are independent SSA values
// %r1 has type i32, %r2 has type f64
```

**Language-level tuple** — a single SSA value of a product type that contains
multiple values. This is a type system concept.

```
%t = call @bar(%x) -> Tuple<i32, f64>
// %t is one SSA value of type Tuple<i32, f64>
%a, %b = unpack %t -> i32, f64
```

A function that returns `(i32, f64)` at the IR level produces two SSA values via
`Return(smallvec![v1, v2])`. A function that returns a single tuple produces one
SSA value via `Return(smallvec![Value::Tuple(vec![v1, v2])])`. If a dialect
implementation confuses these — e.g., the language expects a tuple return but the
IR produces `Return([v1, v2])` — the arity guardrail catches it as an
`ArityMismatch` error.

The new `kirin-unpack` dialect provides DSL-level pack/unpack operations for
languages that use tuples. It is orthogonal to the IR multi-result machinery.

## Continuation Enum

### Before

```rust
pub enum Continuation<V, Ext = Infallible> {
    Continue,
    Jump(Block, Args<V>),
    Fork(SmallVec<[(Block, Args<V>); 2]>),
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Args<V>,
        result: ResultValue,
    },
    Return(V),
    Yield(V),
    Ext(Ext),
}
```

### After

```rust
pub enum Continuation<V, Ext = Infallible> {
    Continue,
    Jump(Block, Args<V>),
    Fork(SmallVec<[(Block, Args<V>); 2]>),
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Args<V>,
        results: SmallVec<[ResultValue; 1]>,
    },
    Return(SmallVec<[V; 1]>),
    Yield(SmallVec<[V; 1]>),
    Ext(Ext),
}
```

All three value-carrying variants change:

| Variant | Before | After | Rationale |
|---------|--------|-------|-----------|
| `Yield` | `Yield(V)` | `Yield(SmallVec<[V; 1]>)` | SCF body blocks can yield multiple values |
| `Return` | `Return(V)` | `Return(SmallVec<[V; 1]>)` | Functions can return multiple SSA values |
| `Call` | `result: ResultValue` | `results: SmallVec<[ResultValue; 1]>` | Callers declare multiple result destinations |

`SmallVec<[T; 1]>` avoids heap allocation for the common single-result case while
supporting N results.

### Routing: Positional Pairing by Parent

The `scf.yield` terminator inside a body block produces raw values — it does not
know the parent operation's result slots. The parent (e.g., `If`, `For`) pairs
yielded values with its result slots **by position** at interpret time.

```rust
// If::interpret
let control = interp.eval_block(stage, block)?;
match control {
    Continuation::Yield(values) => {
        check_arity(values.len(), self.results.len())?;
        for (rv, val) in self.results.iter().zip(values) {
            interp.write(*rv, val)?;
        }
        Ok(Continuation::Continue)
    }
    other => Ok(other),
}
```

This matches MLIR semantics where `scf.yield` operands are positionally matched
to the parent operation's results.

## Arity Guardrails

All Return and Yield sites within a function or SCF body must agree on arity.
Mismatches are **hard errors** (`InterpreterError::ArityMismatch`), not silent
truncation.

### Enforcement Points

| Point | Check | Error |
|-------|-------|-------|
| `run_nested_calls` | `Return.values.len() == Call.results.len()` | ArityMismatch |
| SCF parent interpret impl | `Yield.values.len() == self.results.len()` | ArityMismatch |
| Abstract interpreter `propagate_control` | First return sets arity; subsequent must match | ArityMismatch |
| `bind_block_args` | `args.len() == block.arguments.len()` | ArityMismatch (existing) |

The framework can provide a helper to reduce boilerplate in dialect interpret impls:

```rust
fn write_results(
    interp: &mut impl ValueStore,
    results: &[ResultValue],
    values: SmallVec<[impl Into<V>; 1]>,
) -> Result<(), InterpreterError> {
    if results.len() != values.len() {
        return Err(InterpreterError::ArityMismatch {
            expected: results.len(),
            got: values.len(),
        });
    }
    for (rv, val) in results.iter().zip(values) {
        interp.write(*rv, val.into())?;
    }
    Ok(())
}
```

## Abstract Interpreter

### Product Lattice for Multi-Result

When `V` is a lattice element, multi-result values live in the **product lattice**
`L^n`. This is more precise than packing results into a single `V::Tuple(...)`.

Given lattice `(L, <=, join, meet, bot, top)`:

- **Order**: `(v1,...,vn) <= (w1,...,wn)` iff `vi <= wi` for all i
- **Join**: `(v1,...,vn) join (w1,...,wn) = (v1 join w1, ..., vn join wn)`
- **Widening**: pointwise
- **Bottom**: `(bot, ..., bot)`

Precision comparison:

```
Path 1 returns: result[0] = [1,3],  result[1] = [5,7]
Path 2 returns: result[0] = [2,4],  result[1] = [6,8]

Product lattice (per-index):  result[0] = [1,4], result[1] = [5,8]   -- precise
Flat tuple lattice:           Tuple([1,3],[5,7]) join Tuple([2,4],[6,8]) = top  -- total loss
```

The product lattice tracks each return value independently. A flat tuple lattice
collapses to top when any component differs. This is why `AnalysisResult` stores
`SmallVec<[V; 1]>` (per-index), not `V::Tuple(...)`.

### AnalysisResult Changes

```rust
// Before:
pub struct AnalysisResult<V> {
    pub return_value: Option<V>,
}

// After:
pub struct AnalysisResult<V> {
    pub return_values: Option<SmallVec<[V; 1]>>,
}
```

In `propagate_control`:

```rust
match (&mut result.return_values, new_values) {
    (None, vs) => result.return_values = Some(vs),
    (Some(existing), vs) if existing.len() != vs.len() => {
        return Err(InterpreterError::ArityMismatch {
            expected: existing.len(),
            got: vs.len(),
        }.into());
    }
    (Some(existing), vs) => {
        for (e, v) in existing.iter_mut().zip(vs) {
            *e = e.join(v);
        }
    }
}
```

The function summary type becomes `L^m -> L^n` (m argument lattice values to
n return lattice values). Fixpoint termination is preserved: pointwise widening
on `L^n` terminates if widening on `L` terminates (Bourdoncle 1993).

## Derive Builder Changes

The builder template in `kirin-derive-toolkit` currently rejects `Vec<ResultValue>`
and `Option<ResultValue>` fields with explicit compile errors. This rejection is
lifted. The builder must support three result field collection types:

| Collection | Generated SSA Allocation | Build Result Field |
|------------|-------------------------|--------------------|
| Bare `ResultValue` | `Result(stmt_id, index)` statically | `pub name: ResultValue` |
| `Vec<ResultValue>` | `(0..count).map(\|i\| Result(stmt_id, base + i))` dynamically | `pub name: Vec<ResultValue>` |
| `SmallVec<[ResultValue; N]>` | Same as Vec but collects into SmallVec | `pub name: SmallVec<[ResultValue; N]>` |
| `Option<ResultValue>` | Conditional: `Some(Result(stmt_id, index))` or `None` | `pub name: Option<ResultValue>` |

For `Vec<ResultValue>`, the result count is not known at derive time. The generated
builder function accepts a `count: usize` parameter (or infers it from a related
field like `init_args.len()`).

## Text Format

### Result Names (Statement Level)

Result names are parsed generically at the statement level, not in format strings.
The existing `result_name_list()` parser handles N names:

```
%r1, %r2 = <dialect_op>       // 2 results
%r = <dialect_op>              // 1 result
<dialect_op>                   // 0 results (void)
```

When all ResultValue fields are `Option`, `result_name_list()` becomes optional
(wrapped in `.or_not()`). When any field is `Vec<ResultValue>`, the name count
is dynamic.

### Format String: `Vec<ResultValue>` for Multi-Result Types

A `Vec<ResultValue>` field with `:type` projection parses/prints comma-separated
types. The `Vec` wrapping applies `.separated_by(Comma).collect()` to the type
parser automatically.

```rust
#[chumsky(format = "$call {target}({args}) -> {results:type}")]
pub struct Call<T: CompileTimeValue> {
    target: Symbol,
    args: Vec<SSAValue>,
    results: Vec<ResultValue>,
}
// Text: %r1, %r2 = call @foo(%x, %y) -> i32, f64
```

### Format String: `[...]` Optional Sections

A new `[...]` syntax in format strings defines optional groups. Everything inside
brackets is parsed/printed as an all-or-nothing unit.

**Parser codegen**: contents wrapped in `.or_not()`
**Printer codegen**: contents wrapped in `if field.is_some() { ... }`

```rust
#[chumsky(format = "$if {cond} then {then_body} else {else_body}[ -> {result:type}]")]
pub struct If<T: CompileTimeValue> {
    cond: SSAValue,
    then_body: Block,
    else_body: Block,
    result: Option<ResultValue>,
}
// With result:    %r = if %cond then { yield %x } else { yield %y } -> i32
// Without result: if %cond then { yield %x } else { yield %y }
```

**Validation rules:**
1. Every field inside `[...]` must be `Option<T>` — a required field inside `[...]`
   is a compile error.
2. An `Option` field outside `[...]` is a compile error (ambiguous which tokens
   are optional).
3. `[...]` cannot be nested.
4. Multiple `[...]` sections are independent.

**Escaping**: `[[` produces a literal `[`, `]]` produces a literal `]` (consistent
with `{{`/`}}` for braces).

### Format String: Zero-or-More Results

Combining `Vec<ResultValue>` inside `[...]` handles the 0-to-N case naturally:

```rust
#[chumsky(format = "$for {iv} in {start}..{end} step {step} \
    iter_args({init_args}) do {body}[ -> {results:type}]")]
pub struct For<T: CompileTimeValue> {
    iv: SSAValue,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    init_args: Vec<SSAValue>,
    body: Block,
    results: Vec<ResultValue>,  // 0 to N results; empty Vec when [...] absent
}
```

When `[...]` is absent, a `Vec` field inside receives an empty `Vec`. When present,
the comma-separated list is parsed into the `Vec`.

### Multi-Value Yield and Return

```rust
#[chumsky(format = "$yield {values}")]
pub struct Yield<T: CompileTimeValue> {
    values: Vec<SSAValue>,
}
// Text: yield %a, %b

#[chumsky(format = "$ret {values}")]
pub struct Return<T: CompileTimeValue> {
    values: Vec<SSAValue>,
}
// Text: ret %a, %b
```

## SCF Dialect Changes

### If

```rust
// Before:
pub struct If<T: CompileTimeValue> {
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    result: ResultValue,
}

// After:
#[chumsky(format = "$if {condition} then {then_body} else {else_body}[ -> {results:type}]")]
pub struct If<T: CompileTimeValue> {
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    results: Vec<ResultValue>,  // 0 to N results
}
```

Void-if: `if %cond then { yield } else { yield }` — body yields empty
`Yield(smallvec![])`, parent writes nothing.

Multi-result if: `%a, %b = if %cond then { yield %x, %y } else { yield %p, %q } -> i32, f64`

### For

```rust
// After:
#[chumsky(format = "$for {iv} in {start}..{end} step {step} \
    iter_args({init_args}) do {body}[ -> {results:type}]")]
pub struct For<T: CompileTimeValue> {
    iv: SSAValue,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    init_args: Vec<SSAValue>,
    body: Block,
    results: Vec<ResultValue>,
}
```

Multi-accumulator: init_args provides initial values, body yields updated values
each iteration, final values written to results.

### Yield

```rust
// Before:
pub struct Yield<T: CompileTimeValue> {
    value: SSAValue,
}

// After:
#[chumsky(format = "$yield {values}")]
pub struct Yield<T: CompileTimeValue> {
    values: Vec<SSAValue>,
}
```

`yield` with no arguments produces `Yield(smallvec![])` for void-if bodies.
`yield %a, %b` produces `Yield(smallvec![v_a, v_b])` for multi-result bodies.

## Function Dialect Changes

### Call

```rust
// Before:
pub struct Call<T: CompileTimeValue> {
    target: Symbol,
    args: Vec<SSAValue>,
    res: ResultValue,
}

// After:
#[chumsky(format = "$call {target}({args})[ -> {results:type}]")]
pub struct Call<T: CompileTimeValue> {
    target: Symbol,
    args: Vec<SSAValue>,
    results: Vec<ResultValue>,
}
```

### Return

```rust
// Before:
pub struct Return<T: CompileTimeValue> {
    value: SSAValue,
}

// After:
#[chumsky(format = "$ret {values}")]
pub struct Return<T: CompileTimeValue> {
    values: Vec<SSAValue>,
}
```

## kirin-unpack Dialect

A new dialect providing DSL-level tuple pack/unpack operations. Dialect authors
who want language-level tuple semantics compose this dialect into their language.

The framework provides:
- `MakeTuple` and `Unpack` operation definitions
- Common value type implementations for the stack interpreter
- Common abstract value implementations for the abstract interpreter

Dialect authors implement `Interpretable` on their own value types for custom
typing rules and runtime semantics. The framework implementations serve as
convenience defaults for standard use cases.

This dialect is **orthogonal** to the IR multi-result machinery. A language can
use IR multi-result, language-level tuples via kirin-unpack, or both.

## run_nested_calls Changes

The `run_nested_calls` method changes to handle multi-result Return/Call pairs:

```rust
// Before:
pub(crate) fn run_nested_calls<F>(&mut self, should_exit: F) -> Result<V, E>

// After:
pub(crate) fn run_nested_calls<F>(&mut self, should_exit: F)
    -> Result<SmallVec<[V; 1]>, E>
```

The `pending_results` stack changes from `Vec<ResultValue>` to
`Vec<SmallVec<[ResultValue; 1]>>`. On Return, the values are zipped with the
popped results and written back:

```rust
Continuation::Call { results, .. } => {
    pending_results.push(results.clone());
}
// ...
Continuation::Return(values) => {
    let results = pending_results.pop()
        .ok_or(InterpreterError::NoFrame)?;
    if results.len() != values.len() {
        return Err(InterpreterError::ArityMismatch {
            expected: results.len(),
            got: values.len(),
        }.into());
    }
    for (rv, val) in results.into_iter().zip(values.iter()) {
        ValueStore::write(self, rv, val.clone())?;
    }
}
```

## eval_block Changes

`eval_block` wraps its result in a single-element `Yield` (preserving the existing
contract that body blocks exit via Yield):

```rust
fn eval_block<L: Dialect>(
    &mut self,
    stage: &'ir StageInfo<L>,
    block: Block,
) -> Result<Continuation<V, ConcreteExt>, E> {
    let saved_cursor = self.current_cursor()?;
    let first = block.first_statement(stage);
    self.set_current_cursor(first)?;
    let values = self.run_nested_calls(|_interp, is_yield| is_yield)?;
    self.set_current_cursor(saved_cursor)?;
    Ok(Continuation::Yield(values))  // now SmallVec<[V; 1]>
}
```

## References

- Cousot, P. & Cousot, R. (1977). "Abstract interpretation: a unified lattice
  model for static analysis of programs." POPL.
- Bourdoncle, F. (1993). "Efficient chaotic iteration strategies with widenings."
  FMPA.
- Lattner, C. et al. (2020). "MLIR: A Compiler Infrastructure for the End of
  Moore's Law." arXiv:2002.11054.
