# SCF Dialect Multi-Result Changes

**Finding(s):** W7
**Wave:** 2
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

The SCF dialect (`kirin-scf`) must be updated for multi-result support:

- **If**: `result: ResultValue` -> `results: Vec<ResultValue>` with optional format section `[ -> {results:type}]` to support void-if
- **For**: `result: ResultValue` -> `results: Vec<ResultValue>` with optional format section, multi-accumulator support
- **Yield**: `value: SSAValue` -> `values: Vec<SSAValue>` for multi-value yield

The interpret impls must use arity-checked positional pairing between yielded values and parent result slots.

**Crate(s):** kirin-scf
**File(s):**
- `crates/kirin-scf/src/lib.rs` — struct definitions (lines 54-97)
- `crates/kirin-scf/src/interpret_impl.rs` — Interpretable impls (lines 179-307)
- `crates/kirin-scf/src/tests.rs` — existing tests

**Confidence:** confirmed

## Guiding Principles

- "IR Design Conventions": Block vs Region — SCF operations use `Block` (not `Region`) because MLIR's `scf.if` and `scf.for` have `SingleBlock` traits.
- "Interpreter Conventions": Dialect authors use `I: Interpreter<'ir>`. Arity guardrails: mismatches are hard errors.
- "Derive Infrastructure Conventions": `#[wraps]` is per-variant on `StructuredControlFlow`, so terminator delegation is automatic.
- "Test Conventions": Roundtrip tests go in workspace `tests/roundtrip/<dialect>.rs`. Unit tests go inline.
- "Chumsky Parser Conventions": `[...]` optional sections parsed as all-or-nothing units.

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-scf/src/lib.rs` | modify | Change If.result -> results, For.result -> results, Yield.value -> values, update format strings |
| `crates/kirin-scf/src/interpret_impl.rs` | modify | Multi-result If/For interpret, multi-value Yield, arity checks |
| `crates/kirin-scf/src/tests.rs` | modify | Update existing tests for new field shapes |
| `tests/roundtrip/scf.rs` | modify | Update roundtrip tests for new text format |

**Files explicitly out of scope:**
- `crates/kirin-interpreter/` — Continuation changes done in wave-1
- `crates/kirin-derive-chumsky/` — format DSL `[...]` done in wave-0
- `crates/kirin-derive-toolkit/` — builder template done in wave-0

## Verify Before Implementing

- [ ] **Verify: Wave-0 `[...]` syntax is implemented**
  Run: `grep -n "Optional" crates/kirin-derive-chumsky/src/format.rs`
  Expected: `FormatElement::Optional` variant exists.
  If this fails, STOP — wave-0 format-dsl-plan must complete first.

- [ ] **Verify: Wave-0 builder template accepts Vec<ResultValue>**
  Run: `grep -n "cannot be a Vec" crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs`
  Expected: No matches (rejection removed).
  If matches found, STOP — wave-0 builder-template-plan must complete first.

- [ ] **Verify: Wave-1 Continuation variants are multi-value**
  Run: `grep -n "Yield\|Return" crates/kirin-interpreter/src/control.rs`
  Expected: `Yield(SmallVec<[V; 1]>)` and `Return(SmallVec<[V; 1]>)`.
  If single-value, STOP — wave-1 must complete first.

- [ ] **Verify: write_results helper exists**
  Run: `grep -rn "fn write_results" crates/kirin-interpreter/src/`
  Expected: Helper function exists for arity-checked result writeback.

## Regression Test

- [ ] **Write roundtrip test for void-if**
  In `tests/roundtrip/scf.rs`, add a test that parses and prints:
  ```
  if %cond then { yield } else { yield }
  ```
  This is void-if: no result, no `-> type`. Before changes, this won't parse.

- [ ] **Write roundtrip test for multi-result if**
  ```
  %a, %b = if %cond then { yield %x, %y } else { yield %p, %q } -> i32, f64
  ```

- [ ] **Run tests — confirm they fail**
  Run: `cargo nextest run -p kirin-scf`
  Expected: FAIL — current If struct has single ResultValue.

## Implementation Steps

- [ ] **Step 1: Update Yield struct**
  In `lib.rs` (lines 90-97), change:
  ```rust
  #[chumsky(format = "$yield[ {values}]")]
  pub struct Yield<T: CompileTimeValue> {
      values: Vec<SSAValue>,
      #[kirin(default)]
      marker: std::marker::PhantomData<T>,
  }
  ```
  **Important:** Use `$yield[ {values}]` (with `[...]` optional section), NOT `$yield {values}`. The current codegen for `Vec<T>` uses `.separated_by(Comma).collect()` which requires at least one element. The `[...]` section makes the entire values list optional, mapping absence to `Vec::new()`. This enables void-if bodies where `yield` has no arguments.
  - Text `yield` (no args) -> `values: vec![]`
  - Text `yield %a, %b` -> `values: vec![ssa_a, ssa_b]`

- [ ] **Step 2: Update If struct**
  ```rust
  #[chumsky(format = "$if {condition} then {then_body} else {else_body}[ -> {results:type}]")]
  pub struct If<T: CompileTimeValue> {
      condition: SSAValue,
      then_body: Block,
      else_body: Block,
      results: Vec<ResultValue>,
      #[kirin(default)]
      marker: std::marker::PhantomData<T>,
  }
  ```

- [ ] **Step 3: Update For struct**
  ```rust
  #[chumsky(format = "$for {induction_var} in {start}..{end} step {step} iter_args({init_args}) do {body}[ -> {results:type}]")]
  pub struct For<T: CompileTimeValue> {
      induction_var: SSAValue,
      start: SSAValue,
      end: SSAValue,
      step: SSAValue,
      init_args: Vec<SSAValue>,
      body: Block,
      results: Vec<ResultValue>,
      #[kirin(default)]
      marker: std::marker::PhantomData<T>,
  }
  ```

- [ ] **Step 4: Update Yield interpret impl**
  ```rust
  impl Interpretable for Yield<T> {
      fn interpret<L>(&self, interp: &mut I) -> ... {
          let values: SmallVec<[I::Value; 1]> = self.values
              .iter()
              .map(|ssa| interp.read(*ssa))
              .collect::<Result<_, _>>()?;
          Ok(Continuation::Yield(values))
      }
  }
  ```

- [ ] **Step 5: Update If interpret impl**
  Use `write_results` helper for arity-checked positional pairing:
  ```rust
  match control {
      Continuation::Yield(values) => {
          write_results(interp, &self.results, &values)?;
          Ok(Continuation::Continue)
      }
      other => Ok(other),
  }
  ```
  For Fork (abstract interpretation), both branches are forked — the parent handles result writing after fork resolution.

- [ ] **Step 6: Update For interpret impl**
  Multi-accumulator support. The existing impl (lines 215-270) already uses `carried: Vec<I::Value>` but only handles one value. Changes needed:
  - Line 248: `Continuation::Yield(value)` match -> `Continuation::Yield(values)`. Currently `carried = vec![value]` — change to `carried = values.into_vec()` (or `values.to_vec()`) to capture ALL yielded values as loop-carried state.
  - Lines 250-253: Remove the `if !self.init_args.is_empty()` guard — with multi-result, carried values are always updated from yield.
  - Lines 263-266: Final writeback changes from `interp.write(self.result, value)?` to `write_results(interp, &self.results, &SmallVec::from(carried))?`.
  - Arity check: `init_args.len()` must equal `results.len()` (same number of accumulators and results). Each yield must produce `results.len()` values.

- [ ] **Step 7: Update tests in tests.rs**
  Update existing tests for new field shapes (Vec<ResultValue>, Vec<SSAValue>).

- [ ] **Step 8: Update roundtrip tests**
  In `tests/roundtrip/scf.rs`:
  - Update existing tests for new format
  - Add void-if roundtrip test
  - Add multi-result if roundtrip test
  - Add multi-accumulator for roundtrip test

- [ ] **Step 9: Run all tests**
  Run: `cargo nextest run -p kirin-scf && cargo nextest run --workspace -E 'test(roundtrip::scf)'`
  Expected: All pass.

- [ ] **Step 10: Fix clippy**
  Run: `cargo clippy -p kirin-scf`
  Expected: No warnings.

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations — fix root causes.
- Do NOT leave clippy warnings.
- Do NOT modify kirin-interpreter Continuation enum — that was done in wave-1.
- Do NOT modify kirin-derive-chumsky format DSL — that was done in wave-0.
- No unsafe code.

## Validation

**Final checks:**
```bash
cargo clippy -p kirin-scf                     # Expected: no warnings
cargo nextest run -p kirin-scf                # Expected: all tests pass
cargo nextest run --workspace                  # Expected: no regressions
cargo test --doc -p kirin-scf                 # Expected: all doctests pass
```

**Snapshot tests:** check with `cargo insta test -p kirin-scf` if snapshots exist.

## Success Criteria

1. `If` supports 0-to-N results via `Vec<ResultValue>` with `[...]` optional format section.
2. `For` supports multi-accumulator with `Vec<ResultValue>` results.
3. `Yield` carries `Vec<SSAValue>` for multi-value yield (empty for void-if).
4. Interpret impls use arity-checked positional pairing via `write_results`.
5. Void-if (`if %cond then { yield } else { yield }`) roundtrips correctly.
6. Multi-result if/for roundtrip correctly.
7. All existing tests updated and passing.

**Is this a workaround or a real fix?**
Real fix. This is the full multi-result SCF dialect implementation per the design document.
