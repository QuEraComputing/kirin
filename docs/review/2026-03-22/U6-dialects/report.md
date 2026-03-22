# U6: Dialects Review Report

**Date:** 2026-03-22
**Scope:** 7 dialect crates (~3,233 lines total)
**Reviewer:** Claude Opus 4.6

---

## Executive Summary

The dialect crates are well-structured and consistent. The derive macro infrastructure does heavy lifting -- most dialect definitions are concise and declarative. The Signature migration is complete for `FunctionBody`. The interpreter implementations are clean and use `InterpreterExt` helpers effectively.

The most significant findings are: (1) `scf.for` discards `Yield` values and lacks loop-carried state, limiting its expressiveness vs MLIR's `scf.for`; (2) `scf.if` has no result values, deviating from MLIR's `scf.if` semantics; (3) `From<ArithValue> for i64` silently truncates large values; (4) `ForLoopValue::loop_step` for `i64` can panic on overflow.

**Finding counts:** 0 P0, 3 P1, 7 P2, 5 P3

---

## Cross-Dialect Findings

### [P2] [high confidence] Duplicated `UnitTy` test helper across 6 dialect crates

**Files:**
- `crates/kirin-cf/src/tests.rs:8-15`
- `crates/kirin-scf/src/tests.rs:8-15`
- `crates/kirin-bitwise/src/tests.rs:8-15`
- `crates/kirin-cmp/src/tests.rs:9-16`
- `crates/kirin-function/src/call.rs:37-43`
- `crates/kirin-function/src/ret.rs:21-27`

**Perspective:** Dialect Author / Code Quality

Every dialect's tests define an identical `UnitTy` struct with `Debug, Clone, Hash, PartialEq, Eq, Default` and a `Display` impl printing `"unit"`. Meanwhile, `kirin-test-types` already exports `UnitType` with the same purpose (plus `Lattice`, `Placeholder`, etc.). These test crates already depend on `kirin` (which re-exports `kirin-ir`), so `UnitType` from `kirin-test-types` would only add a dev-dependency. The AGENTS.md rule "put common test tools in `kirin-test-utils`" applies.

**Suggested action:** Add `kirin-test-types` as a dev-dependency to each dialect crate and replace the local `UnitTy` definitions with `UnitType`. This requires `UnitType` to impl `Default` (it already does).

---

### [P2] [high confidence] `__Phantom` variant pattern requires `unreachable!()` in every manual `Interpretable` impl

**Files:**
- `crates/kirin-cf/src/interpret_impl.rs:61`
- `crates/kirin-arith/src/interpret_impl.rs:64`
- `crates/kirin-bitwise/src/interpret_impl.rs:48`
- `crates/kirin-cmp/src/interpret_impl.rs:263`

**Perspective:** Code Quality / Soundness Adversary

The `#[non_exhaustive]` + `__Phantom(PhantomData<T>)` pattern for carrying the type parameter requires every hand-written `Interpretable` impl to include a `Self::__Phantom(..) => unreachable!()` arm. This is pure boilerplate and a latent footgun -- if a new variant were added the compiler would catch it, but the `unreachable!()` invites copy-paste without thought. The SCF dialect avoids this by using `#[wraps]` with separate struct types.

Note: The design context says PhantomData for type parameters is intentional, so this is not about the PhantomData itself but about the repeated `unreachable!()` arms it forces.

**Suggested action:** Consider whether `#[derive(Interpretable)]` could auto-generate the `__Phantom => unreachable!()` arm, or whether the `#[wraps]` struct-per-variant pattern (as in SCF, function) should be the recommended approach for new dialects.

---

### [P3] [high confidence] Inconsistent `#[non_exhaustive]` usage across dialects

**Files:**
- `crates/kirin-cf/src/lib.rs:28` -- has `#[non_exhaustive]`
- `crates/kirin-arith/src/lib.rs:84` -- has `#[non_exhaustive]`
- `crates/kirin-bitwise/src/lib.rs:53` -- has `#[non_exhaustive]`
- `crates/kirin-cmp/src/lib.rs:14` -- has `#[non_exhaustive]`
- `crates/kirin-scf/src/lib.rs:40` -- no `#[non_exhaustive]` (uses `#[wraps]`)
- `crates/kirin-function/src/lib.rs:36` -- no `#[non_exhaustive]` (uses `#[wraps]`)

**Perspective:** Dialect Author

The pattern is actually consistent within two categories: flat enums with `__Phantom` use `#[non_exhaustive]`, while `#[wraps]` enums do not. This is correct behavior -- `#[wraps]` enums have no `__Phantom` and their variants are individually typed structs. Documenting this pattern would help future dialect authors understand the distinction.

**Suggested action:** Informational only. Consider adding a note in AGENTS.md under "Derive Infrastructure Conventions" explaining when `#[non_exhaustive]` is expected.

---

### [P3] [medium confidence] Test coverage for `If` and `For` dialect property tests is minimal

**Files:**
- `crates/kirin-scf/src/tests.rs` -- only tests `Yield` and `StructuredControlFlow::Yield`

**Perspective:** Dialect Author / Code Quality

The SCF tests only exercise `Yield`. There are no tests for `If` or `For` dialect properties (arguments, results, blocks, terminator status, etc.). The CF, bitwise, and cmp crates each have thorough property tests for every variant. `If` should test that it has 1 argument (condition), 2 blocks (then/else), 0 results, and is not a terminator. `For` should test its 4 arguments (induction_var, start, end, step), 1 block (body), and non-terminator status.

**Suggested action:** Add dialect property tests for `If` and `For` following the pattern in the other dialect test files.

---

## kirin-scf Findings

### [P1] [high confidence] `scf.for` discards `Yield` values -- no loop-carried state

**File:** `crates/kirin-scf/src/interpret_impl.rs:206-207`

**Perspective:** Formalism / Soundness Adversary

In MLIR, `scf.for` supports loop-carried values (accumulators) via `iter_args` and `init_args`. The `yield` at the end of each iteration provides the new values for the next iteration's block arguments, and the final yield values become the `scf.for` results. In Kirin's implementation:

```rust
match control {
    Continuation::Yield(_) => {}  // value is discarded
    other => return Ok(other),
}
```

The yielded value is silently discarded. This means `scf.for` cannot express reductions, prefix sums, or any accumulation pattern. The `For` struct also has no `ResultValue` field and no `init_args`, confirming this is a design gap rather than a bug.

This significantly limits `scf.for`'s expressiveness compared to MLIR. A language using Kirin's SCF dialect would need to use mutable state or restructure control flow to express patterns that MLIR's `scf.for` handles natively.

**Suggested action:** Track as a design gap. When loop-carried values are needed, extend `For` with `init_args: Vec<SSAValue>`, a `result: Vec<ResultValue>`, and thread the `Yield` value back as block arguments for the next iteration.

---

### [P1] [high confidence] `scf.if` has no result values -- cannot be used as an expression

**File:** `crates/kirin-scf/src/lib.rs:54-63`

**Perspective:** Formalism

In MLIR, `scf.if` can produce results: the `yield` in each branch provides the if-expression's result values. Kirin's `If` struct has no `ResultValue` field. The interpreter implementation returns `Continuation::Jump` to the appropriate block, but there's no mechanism to capture the yielded value from the taken branch.

This means `scf.if` in Kirin can only be used for side effects, not as a value-producing expression. Combined with the `scf.for` gap above, this reduces the SCF dialect to a control-flow-only tool rather than the expression-oriented structured control flow it models in MLIR.

**Suggested action:** Add a `result: ResultValue` (or `results: Vec<ResultValue>`) field to `If`. The interpreter should evaluate the chosen branch's block, capture the `Yield` value, write it to the result, and return `Continuation::Continue`.

---

### [P1] [medium confidence] `ForLoopValue::loop_step` for `i64` can panic on overflow

**File:** `crates/kirin-scf/src/interpret_impl.rs:26-28`

**Perspective:** Soundness Adversary

```rust
fn loop_step(&self, step: &i64) -> i64 {
    self + step
}
```

This uses `i64::add` which panics in debug mode and wraps in release mode on overflow. For a compiler IR interpreter, a panic during interpretation is unacceptable -- it should return an error. If `iv` is near `i64::MAX` and `step` is positive, or near `i64::MIN` with a negative step, this will panic.

The `loop_condition` method is safe (comparison cannot overflow), but `loop_step` is not.

**Suggested action:** Use `checked_add` (returning `Option<Self>`) and propagate the overflow as an interpreter error, similar to how `kirin-arith` uses `CheckedDiv`/`CheckedRem`. This requires changing the `ForLoopValue::loop_step` return type to `Option<Self>` or `Result<Self, E>`.

---

### [P2] [medium confidence] `For` interpreter does not write `induction_var` SSAValue

**File:** `crates/kirin-scf/src/interpret_impl.rs:199-204`

**Perspective:** Formalism / Soundness Adversary

The `For` struct has an `induction_var: SSAValue` field, and the parser format includes it (`$for {induction_var} in {start}..{end} step {step} do {body}`). However, the interpreter implementation never writes to `self.induction_var`. Instead, it reads `self.start` to initialize `iv` and passes `iv` as a block argument via `bind_block_args`.

The `induction_var` field appears to be used only by the parser to bind the variable name in the text format. If this is correct, it should be documented. If `induction_var` is intended to be written as an SSA value (so code outside the loop body can reference the final IV value), then the interpreter has a bug.

**Suggested action:** Clarify whether `induction_var` serves only as a parser binding (the block argument carries the actual value) or should be written by the interpreter. If the former, add a doc comment explaining this. If the latter, write to `self.induction_var` after the loop exits.

---

## kirin-arith Findings

### [P2] [high confidence] `From<ArithValue> for i64` silently truncates large values

**File:** `crates/kirin-arith/src/types/arith_value.rs:159-176`

**Perspective:** Soundness Adversary

```rust
impl From<ArithValue> for i64 {
    fn from(v: ArithValue) -> Self {
        match v {
            ArithValue::I128(x) => x as i64,  // truncates
            ArithValue::U64(x) => x as i64,   // wraps
            ArithValue::U128(x) => x as i64,  // truncates
            ArithValue::F32(x) => x as i64,   // saturating in Rust 2024
            ArithValue::F64(x) => x as i64,   // saturating in Rust 2024
            // ...
        }
    }
}
```

Several arms silently truncate or wrap: `I128 -> i64`, `U64 -> i64` (values > i64::MAX wrap to negative), `U128 -> i64`. Float conversions use saturating semantics in Rust 2024 edition, which is at least defined behavior. This `From` impl is used as a convenience conversion but could mask real bugs in downstream code that expects lossless conversion.

**Suggested action:** Replace with `TryFrom<ArithValue> for i64` that returns an error for out-of-range values. If the lossy conversion is intentionally needed somewhere, keep it but rename to a method like `to_i64_lossy()` so callers opt in explicitly.

---

### [P3] [low confidence] `#[allow(clippy::derivable_impls)]` on `ArithType::Default`

**File:** `crates/kirin-arith/src/types/arith_type.rs:50-55`

**Perspective:** Code Quality

The `Default` impl for `ArithType` returns `Self::I64`, which is the first variant. Clippy correctly identifies this as derivable. The `#[allow]` suppresses the lint, which is fine if the intent is to be explicit about the default rather than relying on variant order. However, a comment explaining why `I64` is chosen as the default would be helpful.

**Suggested action:** Add a brief comment: `// I64 is the default width for untyped integer literals`.

---

## kirin-bitwise Findings

### [P2] [medium confidence] `Shl` and `Shr` interpreter can panic on large shift amounts

**File:** `crates/kirin-bitwise/src/interpret_impl.rs:42-47`

**Perspective:** Soundness Adversary

```rust
Bitwise::Shl { lhs, rhs, result, .. } => interp.binary_op(*lhs, *rhs, *result, |a, b| a << b),
Bitwise::Shr { lhs, rhs, result, .. } => interp.binary_op(*lhs, *rhs, *result, |a, b| a >> b),
```

For `i64`, shifting by 64 or more bits panics in debug mode. Unlike `Div`/`Rem` in `kirin-arith` which use `CheckedDiv`/`CheckedRem`, shifts have no checked variant here. The doc comment in `lib.rs:42` notes "Verifier passes are expected to enforce type compatibility" but there is no verifier yet.

This is consistent with why `Shl`/`Shr` are not marked `#[kirin(speculatable)]` -- the dialect acknowledges they can fail. But the panic-on-overflow should be a returned error, not a process crash.

**Suggested action:** Use `try_binary_op` with checked shift operations (e.g., `i64::checked_shl` / `i64::checked_shr`) similar to `kirin-arith`'s division handling. Define `CheckedShl`/`CheckedShr` traits.

---

## kirin-cmp Findings

### [P3] [medium confidence] `CompareValue` impl only for `i64` -- no float comparison support

**File:** `crates/kirin-cmp/src/interpret_impl.rs:22-43`

**Perspective:** Formalism / Dialect Author

`CompareValue` is only implemented for `i64`. There is no impl for `f32`, `f64`, or `ArithValue`. This means `Cmp` with float operands cannot be interpreted. Since `kirin-arith` provides `ArithType` with float variants and the text format supports `$lt %a, %b -> f64`, users might expect float comparisons to work.

Float comparison has subtleties (NaN handling, total ordering vs partial ordering) that may justify deferring this, but it should be documented as a known limitation.

**Suggested action:** Either add `CompareValue` impls for `f32`/`f64` (using IEEE 754 partial ordering, returning 0 for NaN comparisons), or document this as a known gap in the crate-level docs.

---

## kirin-function Findings

### [P2] [medium confidence] `Lambda` has no `Signature` field -- inconsistent with `FunctionBody`

**File:** `crates/kirin-function/src/lambda.rs:7-17`

**Perspective:** Formalism / Dialect Author

`FunctionBody` has a `sig: Signature<T>` field (line 13 of body.rs), which enables `HasSignature` derivation. `Lambda` does not have a `Signature` field. In PL theory, lambdas have types (function types with parameter and return types). The `Lambda` struct has a `res: ResultValue` but no way to express the parameter types of the lambda.

This means the framework cannot introspect a lambda's type signature, which limits type-checking and specialization passes. If the lambda's signature is inferred from the body's block arguments, this should be documented.

**Suggested action:** Either add `sig: Signature<T>` to `Lambda` (matching `FunctionBody`), or document why lambda signatures are intentionally omitted (e.g., "lambda signatures are inferred from block argument types and the return type annotation").

---

### [P3] [low confidence] `Bind` interpreter returns an error -- unimplemented

**File:** `crates/kirin-function/src/interpret_impl.rs:126-143`

**Perspective:** Dialect Author

`Bind::interpret` unconditionally returns an error with message "bind is not yet supported in the interpreter". This is the only unimplemented interpreter in all 7 dialect crates. While `Lifted` (which contains `Bind`) is likely newer and less tested, having an unimplemented operation in a released dialect crate could confuse users.

**Suggested action:** Either implement `Bind` interpretation (create a closure value from the target function + captures) or document the limitation prominently in the `Bind` struct docs.

---

### [P2] [high confidence] `kirin-function/src/tests.rs` is effectively empty

**File:** `crates/kirin-function/src/tests.rs:1-8`

**Perspective:** Code Quality

The `tests.rs` file for `kirin-function` contains only comments explaining that tests cannot be written because `FunctionBody` and `Lambda` have private fields. The sub-modules (`call.rs`, `ret.rs`) have their own inline tests, but `Lexical`, `Lifted`, `FunctionBody`, and `Lambda` have no integration-level tests for `#[wraps]` delegation (beyond what the empty file comments mention).

**Suggested action:** Either add `#[cfg(test)]` constructor helpers within the crate (not public) to enable wrapper delegation tests, or add roundtrip tests in the workspace `tests/roundtrip/` directory that exercise `Lexical` and `Lifted` end-to-end.

---

## Summary Table

| ID | Sev | Confidence | Dialect | Title |
|----|-----|-----------|---------|-------|
| 1 | P1 | high | kirin-scf | `scf.for` discards Yield values, no loop-carried state |
| 2 | P1 | high | kirin-scf | `scf.if` has no result values |
| 3 | P1 | medium | kirin-scf | `ForLoopValue::loop_step` can panic on overflow |
| 4 | P2 | high | cross-dialect | Duplicated `UnitTy` test helper |
| 5 | P2 | high | cross-dialect | `__Phantom` forces `unreachable!()` boilerplate |
| 6 | P2 | high | kirin-arith | `From<ArithValue> for i64` silently truncates |
| 7 | P2 | medium | kirin-scf | `For` interpreter ignores `induction_var` SSAValue |
| 8 | P2 | medium | kirin-bitwise | Shift operations can panic on large amounts |
| 9 | P2 | medium | kirin-function | `Lambda` missing `Signature` field |
| 10 | P2 | high | kirin-function | `tests.rs` is effectively empty |
| 11 | P3 | high | cross-dialect | Inconsistent `#[non_exhaustive]` (actually consistent by pattern) |
| 12 | P3 | low | kirin-arith | `#[allow(clippy::derivable_impls)]` without explanation |
| 13 | P3 | medium | kirin-cmp | `CompareValue` only for `i64`, no float support |
| 14 | P3 | low | kirin-function | `Bind` interpreter unimplemented |
| 15 | P3 | medium | kirin-scf | Missing dialect property tests for `If` and `For` |
