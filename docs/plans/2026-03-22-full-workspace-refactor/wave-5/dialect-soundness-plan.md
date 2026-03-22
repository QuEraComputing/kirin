# Dialect Soundness Fixes

**Finding(s):** P2 (ArithValue TryFrom, checked shifts, Lambda Signature)
**Wave:** 5
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

### P2: `From<ArithValue> for i64` silently truncates large values

**File:** `crates/kirin-arith/src/types/arith_value.rs:159-176`

Several arms silently truncate or wrap: `I128 -> i64`, `U64 -> i64` (values > i64::MAX wrap to negative), `U128 -> i64`. This `From` impl is used as a convenience conversion but could mask real bugs in downstream code.

**Action:** Replace with `TryFrom<ArithValue> for i64` that returns an error for out-of-range values.

### P2: `Shl` and `Shr` interpreter can panic on large shift amounts

**File:** `crates/kirin-bitwise/src/interpret_impl.rs:42-47`

Shifting by 64 or more bits panics in debug mode. Unlike `Div`/`Rem` in `kirin-arith` which use `CheckedDiv`/`CheckedRem`, shifts have no checked variant.

**Action:** Use `try_binary_op` with checked shift operations.

### P2: `Lambda` has no `Signature` field -- inconsistent with `FunctionBody`

**File:** `crates/kirin-function/src/lambda.rs:7-17`

`FunctionBody` has a `sig: Signature<T>` field enabling `HasSignature` derivation. `Lambda` does not. In PL theory, lambdas have types (function types with parameter and return types).

**Action:** Either add `sig: Signature<T>` to `Lambda` or document why lambda signatures are intentionally omitted.

**Why grouped:** All three are dialect soundness improvements in separate crates (kirin-arith, kirin-bitwise, kirin-function). They are independent but share the same wave because they're all breaking dialect changes.

**Crate(s):** kirin-arith, kirin-bitwise, kirin-function
**File(s):**
- `crates/kirin-arith/src/types/arith_value.rs`
- `crates/kirin-bitwise/src/interpret_impl.rs`
- `crates/kirin-function/src/lambda.rs`
**Confidence:** high (ArithValue), medium (shifts, Lambda)

## Guiding Principles

- "Dialect developer contract" -- parser, pretty print, and interpreter are ALL required for dialect authors.
- "No unsafe code."
- Dialect domain: ArithValue TryFrom relates to type promotion rules and overflow semantics. Checked shifts relate to defined behavior for all inputs. Lambda Signature relates to function application and parametric polymorphism.

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-arith/src/types/arith_value.rs` | modify | Replace `From<ArithValue> for i64` with `TryFrom`; add `to_i64_lossy()` if needed |
| `crates/kirin-bitwise/src/interpret_impl.rs` | modify | Use `try_binary_op` with checked shifts for Shl/Shr |
| `crates/kirin-function/src/lambda.rs` | modify | Add `sig: Signature<T>` field or document the intentional omission |

**Files explicitly out of scope:**
- `crates/kirin-scf/` -- covered by scf-result-values-plan
- `crates/kirin-cmp/` -- CompareValue for floats is P3, low priority

## Verify Before Implementing

- [ ] **Verify: `From<ArithValue> for i64` exists and truncates**
  Run: Read `crates/kirin-arith/src/types/arith_value.rs` lines 159-176
  Expected: See `as i64` casts that truncate/wrap

- [ ] **Verify: callers of `From<ArithValue> for i64`**
  Run: Grep for `i64::from` or `Into::<i64>` or `.into()` patterns that use this impl in the workspace
  Expected: Identify all call sites that need updating to `try_from`/`try_into`

- [ ] **Verify: checked_shl/checked_shr exist on i64**
  Run: Confirm `i64::checked_shl(u32)` and `i64::checked_shr(u32)` exist
  Expected: Standard library provides these methods

- [ ] **Verify: Lambda struct current definition**
  Run: Read `crates/kirin-function/src/lambda.rs` lines 7-17
  Expected: No `sig` field

- [ ] **Verify: FunctionBody has sig field for reference**
  Run: Read `crates/kirin-function/src/body.rs` (or wherever FunctionBody is defined)
  Expected: Has `sig: Signature<T>` field

## Regression Test (P0/P1 findings)

Not required for P2 findings. Tests will be written as part of implementation.

## Implementation Steps

### Part A: ArithValue TryFrom

- [ ] **Step 1: Replace `From<ArithValue> for i64` with `TryFrom`**
  Change the impl to `TryFrom<ArithValue> for i64` with `type Error = ArithConversionError` (or a suitable error type). Each arm that could lose data should return `Err`. Keep the `I64` arm as infallible. For float arms, use `as i64` with range checking.

- [ ] **Step 2: Add `to_i64_lossy()` method if needed**
  If any call site genuinely needs the lossy behavior, add a method `to_i64_lossy(&self) -> i64` that preserves the old `as i64` behavior but makes the intent explicit.

- [ ] **Step 3: Update all call sites**
  Find and update all call sites from `.into()` to `.try_into().expect(...)` or proper error handling.

- [ ] **Step 4: Run arith tests**
  Run: `cargo nextest run -p kirin-arith`
  Expected: All tests pass

### Part B: Checked Shifts

- [ ] **Step 5: Define CheckedShl/CheckedShr traits or use try_binary_op directly**
  In `crates/kirin-bitwise/src/interpret_impl.rs`, the current implementation is generic over `I::Value` (not `i64`). The trait bounds require `I::Value: Shl<Output = I::Value> + Shr<Output = I::Value>`.

  **Approach A (new trait bounds):** Define `CheckedShl` and `CheckedShr` traits (e.g., in kirin-bitwise or kirin-interpreter) that return `Option` or `Result`. Add bounds to the `Interpretable` impl and use `try_binary_op`:
  ```rust
  interp.try_binary_op(*lhs, *rhs, *result, |a, b| {
      a.checked_shl(b).ok_or("shift overflow")
  })
  ```
  This is a breaking change for downstream `I::Value` implementors (they must also implement `CheckedShl`/`CheckedShr`).

  **Approach B (document-only for generic, fix for concrete):** If adding new trait bounds is too disruptive, keep the generic `<<`/`>>` but add documentation noting that shift overflow is the caller's responsibility. Then add a concrete `i64` implementation in the test suite that demonstrates checked shifts.

  Choose Approach A if the trait change is feasible; choose Approach B if Approach A breaks too many downstream types. Adapt the error type to match `try_binary_op`'s signature (see kirin-arith's `CheckedDiv`/`CheckedRem` usage for reference).

- [ ] **Step 6: Add tests for shift edge cases**
  Add tests for shift by 0, 63, 64, negative shift amount, and `i64::MAX` shift.

- [ ] **Step 7: Run bitwise tests**
  Run: `cargo nextest run -p kirin-bitwise`
  Expected: All tests pass

### Part C: Lambda Signature

- [ ] **Step 8: Add `sig: Signature<T>` to Lambda or document intentional omission**
  Read the Lambda usage patterns first. If Lambda is used primarily for closures where the signature is inferred from block args, adding a Signature field may be the wrong approach. In that case, add documentation explaining the design choice.

  If adding the field: update the struct, derive macros should handle parser/printer. Update any constructors/tests.

- [ ] **Step 9: Run function tests**
  Run: `cargo nextest run -p kirin-function && cargo nextest run --workspace`
  Expected: All tests pass

- [ ] **Step 10: Run clippy on all three crates**
  Run: `cargo clippy -p kirin-arith && cargo clippy -p kirin-bitwise && cargo clippy -p kirin-function`
  Expected: No warnings

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations.
- Do NOT change `ArithValue` enum variants -- only the `From`/`TryFrom` conversion.
- Do NOT remove `From<i64> for ArithValue` (the reverse direction is fine).
- Do NOT modify kirin-scf (separate plan).
- No unsafe code.

## Validation

**Per-step checks:**
- After step 1-3: `cargo build --workspace` -- Expected: compiles (may need call site updates)
- After step 5: `cargo check -p kirin-bitwise` -- Expected: compiles
- After step 8: `cargo check -p kirin-function` -- Expected: compiles

**Final checks:**
```bash
cargo clippy -p kirin-arith                  # Expected: no warnings
cargo clippy -p kirin-bitwise                # Expected: no warnings
cargo clippy -p kirin-function               # Expected: no warnings
cargo nextest run -p kirin-arith             # Expected: all tests pass
cargo nextest run -p kirin-bitwise           # Expected: all tests pass
cargo nextest run -p kirin-function          # Expected: all tests pass
cargo nextest run --workspace                # Expected: no regressions
```

**Snapshot tests:** No.

## Success Criteria

1. `ArithValue` to `i64` conversion is fallible (`TryFrom`), no silent truncation.
2. Shift operations use checked arithmetic and return errors for out-of-range shifts.
3. Lambda either has a Signature field or its omission is documented.
4. All call sites of the old `From<ArithValue> for i64` are updated.
5. No regressions.

**Is this a workaround or a real fix?**
These are real fixes. TryFrom replaces the silently lossy From. Checked shifts replace panicking shifts. Lambda Signature is an additive enhancement (or documentation if omission is intentional).
