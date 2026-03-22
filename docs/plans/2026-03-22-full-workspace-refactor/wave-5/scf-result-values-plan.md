# SCF Result Values and Loop Overflow Fix

**Finding(s):** P1-19, P1-20, P1-21
**Wave:** 5
**Agent role:** Implementer
**Estimated effort:** design-work

---

## Issue

### P1-19: `scf.for` discards Yield values -- no loop-carried state

**File:** `crates/kirin-scf/src/interpret_impl.rs:206-207`

In MLIR, `scf.for` supports loop-carried values (accumulators) via `iter_args` and `init_args`. The `yield` at the end of each iteration provides the new values for the next iteration's block arguments, and the final yield values become the `scf.for` results. In Kirin:

```rust
match control {
    Continuation::Yield(_) => {}  // value is discarded
    other => return Ok(other),
}
```

The yielded value is silently discarded. The `For` struct also has no `ResultValue` field and no `init_args`, confirming this is a design gap.

### P1-20: `scf.if` has no result values -- cannot be used as expression

**File:** `crates/kirin-scf/src/lib.rs:54-63`

In MLIR, `scf.if` can produce results: the `yield` in each branch provides the if-expression's result values. Kirin's `If` struct has no `ResultValue` field. The interpreter returns `Continuation::Jump` to the appropriate block, but there's no mechanism to capture the yielded value from the taken branch.

### P1-21: `ForLoopValue::loop_step` for i64 can panic on overflow

**File:** `crates/kirin-scf/src/interpret_impl.rs:26-28`

`self + step` uses unchecked `i64::add` which panics in debug and wraps in release on overflow.

**Why grouped:** All three are in kirin-scf. P1-19 and P1-20 are the design gap (SCF result values). P1-21 is a simpler fix that touches the same file. The design changes in P1-19/P1-20 are breaking -- they add fields to `If` and `For` structs.

**Crate(s):** kirin-scf
**File(s):**
- `crates/kirin-scf/src/lib.rs`
- `crates/kirin-scf/src/interpret_impl.rs`
**Confidence:** high (P1-19, P1-20), medium (P1-21)

## Guiding Principles

- "IR Design Conventions" -- `Block` vs `Region`: SCF operations use `Block` fields for their bodies (SingleBlock in MLIR terms).
- "`BlockInfo::terminator` is a cached pointer" -- the terminator field is NOT a separate statement.
- "Interpreter Conventions" -- `Continuation::Yield` carries values to return from a block/region. `ResultValue` fields hold operation results.
- "Dialect developer contract" -- parser, pretty print, and interpreter are ALL required for dialect authors.
- SCF domain context: Structured control flow (Cytron et al.), loop nesting, induction variables.

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-scf/src/lib.rs` | modify | Add `result: ResultValue` to `If`; add `init_args: Vec<SSAValue>`, `result: Vec<ResultValue>` to `For`; update `ForLoopValue` trait |
| `crates/kirin-scf/src/interpret_impl.rs` | modify | Implement result value capture for If; implement loop-carried state for For; use checked_add for loop_step |
| `crates/kirin-scf/src/tests.rs` | modify | Add tests for If result values, For loop-carried state, ForLoopValue overflow |
| `tests/roundtrip/scf.rs` | modify | Update roundtrip tests for new If/For text format |

**Files explicitly out of scope:**
- Parser and printer changes for If/For -- the `#[derive(HasParser, PrettyPrint)]` macros should auto-generate these from the struct definition. If not, additive parser/printer work may be needed but is tracked separately.

## Verify Before Implementing

- [ ] **Verify: current If struct definition**
  Run: Read `crates/kirin-scf/src/lib.rs` lines 54-63
  Expected: `If` has `condition: SSAValue`, `then_body: Block`, `else_body: Block` but NO `result`

- [ ] **Verify: current For struct definition**
  Run: Read `crates/kirin-scf/src/lib.rs` lines 30-50 (approximate)
  Expected: `For` has `induction_var`, `start`, `end`, `step`, `body` but NO `init_args`/`result`

- [ ] **Verify: ForLoopValue trait definition**
  Run: Grep for `trait ForLoopValue` in `crates/kirin-scf/src/`
  Expected: `loop_step` returns `Self` (not `Option<Self>`)

- [ ] **Verify: derive macros handle new fields automatically**
  Run: `cargo check -p kirin-scf` after adding fields (step 1)
  Expected: Compiles -- derive macros generate parser/printer for new fields
  If this fails, manual parser/printer updates are needed.

## Regression Test (P0/P1 findings)

- [ ] **Write test for P1-21: ForLoopValue overflow**
  Create a test that calls `loop_step` with values near `i64::MAX` and a positive step. Currently panics in debug; the fix should return `None` or `Err`.
  Test file: `crates/kirin-scf/src/tests.rs` or `interpret_impl.rs` inline test

- [ ] **Run the test -- confirm it panics (demonstrates the issue)**
  Run: `cargo nextest run -p kirin-scf -E 'test(loop_step_overflow)'`
  Expected: FAIL (panic in debug mode)

## Implementation Steps

### Phase A: P1-21 (simple fix, no design work)

- [ ] **Step 1: Change `ForLoopValue::loop_step` to return `Option<Self>`**
  In `crates/kirin-scf/src/interpret_impl.rs`, change the trait definition:
  ```rust
  fn loop_step(&self, step: &Self) -> Option<Self>;
  ```
  Update the `i64` impl to use `self.checked_add(*step)`.

- [ ] **Step 2: Update For interpreter to handle None from loop_step**
  Where `loop_step` is called, handle the `None` case by returning an `InterpreterError::Custom` with a descriptive message about arithmetic overflow.

- [ ] **Step 3: Run overflow regression test**
  Run: `cargo nextest run -p kirin-scf -E 'test(loop_step_overflow)'`
  Expected: PASS

### Phase B: P1-20 (If result values -- simpler)

- [ ] **Step 4: Add `result: ResultValue` field to `If` struct**
  In `crates/kirin-scf/src/lib.rs`, add a `result: ResultValue` field to `If`. This is a breaking change -- all constructors/patterns will need updating.

- [ ] **Step 5: Update If interpreter to capture Yield and write result**
  In the interpreter, after evaluating the chosen branch's block:
  1. If the block returns `Continuation::Yield(values)`, write the first value to `self.result`
  2. Return `Continuation::Continue` (the If is now a value-producing expression)
  3. If the block returns something else (Jump, Return), propagate it

- [ ] **Step 6: Update tests and roundtrip for If**
  Update `tests/roundtrip/scf.rs` to include the result value in the text format. Add interpreter tests verifying If produces a value.

### Phase C: P1-19 (For loop-carried state -- more complex)

- [ ] **Step 7: Add `init_args` and `results` fields to `For` struct**
  In `crates/kirin-scf/src/lib.rs`, add:
  - `init_args: Vec<SSAValue>` -- initial values for loop-carried state
  - `results: Vec<ResultValue>` -- where the final loop values are written

- [ ] **Step 8: Update For interpreter for loop-carried state**
  The interpreter loop should:
  1. Initialize loop-carried values from `init_args`
  2. On each iteration, bind loop-carried values as additional block arguments
  3. When `Continuation::Yield(values)` is returned, use those values as the next iteration's loop-carried args
  4. After the loop exits, write the final loop-carried values to `self.results`

- [ ] **Step 9: Update tests and roundtrip for For**
  Add a roundtrip test for `For` with init_args and results. Add an interpreter test for a simple accumulator (e.g., sum 1..10).

- [ ] **Step 10: Run all tests**
  Run: `cargo nextest run -p kirin-scf && cargo nextest run --workspace`
  Expected: All tests pass

- [ ] **Step 11: Run clippy**
  Run: `cargo clippy -p kirin-scf`
  Expected: No warnings

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations.
- Do NOT change the `Yield` struct -- it is correct as-is (carries values up to the parent).
- Do NOT modify other dialect crates (kirin-cf, kirin-arith, etc.).
- Do NOT remove existing tests -- only add or update.
- No unsafe code.
- Do NOT break the existing `StructuredControlFlow` wrapping enum pattern if it uses `#[wraps]`.

## Validation

**Per-step checks:**
- After step 1-2: `cargo check -p kirin-scf` -- Expected: compiles
- After step 4-5: `cargo check -p kirin-scf` -- Expected: compiles
- After step 7-8: `cargo check -p kirin-scf` -- Expected: compiles

**Final checks:**
```bash
cargo clippy -p kirin-scf                    # Expected: no warnings
cargo nextest run -p kirin-scf               # Expected: all tests pass
cargo nextest run --workspace                # Expected: no regressions
cargo test --doc -p kirin-scf                # Expected: all doctests pass
```

**Snapshot tests:** If insta snapshots exist, run `cargo insta test -p kirin-scf` and report changes.

## Success Criteria

1. `scf.if` can produce result values -- Yield from branches writes to ResultValue.
2. `scf.for` supports loop-carried state -- init_args provide initial values, Yield feeds back, results capture final values.
3. `ForLoopValue::loop_step` uses checked arithmetic and returns `Option<Self>`.
4. Text format roundtrips for both If and For with the new fields.
5. All existing tests pass (with updates for the new struct fields).

**Is this a workaround or a real fix?**
This is the real fix. The SCF dialect is being brought closer to MLIR's `scf.if` and `scf.for` semantics. The overflow fix (P1-21) is also definitive. These are breaking changes that expand the dialect's expressiveness.
