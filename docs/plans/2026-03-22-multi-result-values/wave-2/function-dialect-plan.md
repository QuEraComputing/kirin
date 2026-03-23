# Function Dialect Multi-Result Changes

**Finding(s):** W8
**Wave:** 2
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

The function dialect (`kirin-function`) must be updated for multi-result support:

- **Call**: `res: ResultValue` -> `results: Vec<ResultValue>` with optional format section `[ -> {results:type}]` to support void calls
- **Return**: `value: SSAValue` -> `values: Vec<SSAValue>` for multi-value return

The interpret impls must produce `SmallVec`-wrapped Continuation variants and use the `results` field (plural) for Call.

**Crate(s):** kirin-function
**File(s):**
- `crates/kirin-function/src/call.rs` — Call struct (lines 1-12)
- `crates/kirin-function/src/ret.rs` — Return struct (lines 1-10)
- `crates/kirin-function/src/interpret_impl.rs` — Interpretable impls for Call (lines 145-256) and Return (lines 258-273)
- `crates/kirin-function/src/lib.rs` — re-exports, may need updating

**Confidence:** confirmed

## Guiding Principles

- "Interpreter Conventions": Dialect authors use `I: Interpreter<'ir>`. `Continuation::Call` carries `results: SmallVec<[ResultValue; 1]>`.
- "No unsafe code": All implementations MUST use safe Rust.
- "Test Conventions": Roundtrip tests go in `tests/roundtrip/function.rs`. Unit tests go inline.

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-function/src/call.rs` | modify | `res: ResultValue` -> `results: Vec<ResultValue>`, update format, update accessors |
| `crates/kirin-function/src/ret.rs` | modify | `value: SSAValue` -> `values: Vec<SSAValue>`, update format |
| `crates/kirin-function/src/interpret_impl.rs` | modify | Update Call/Return interpret impls for multi-result |
| `crates/kirin-function/src/lib.rs` | possibly modify | Check if Lexical/Lifted enums need accessor updates |
| `tests/roundtrip/function.rs` | modify | Update roundtrip tests |

**Files explicitly out of scope:**
- `crates/kirin-interpreter/` — Continuation changes done in wave-1
- `crates/kirin-scf/` — SCF changes in separate wave-2 plan

## Verify Before Implementing

- [ ] **Verify: Wave-1 Continuation::Call has `results` (plural)**
  Run: `grep -n "results:" crates/kirin-interpreter/src/control.rs`
  Expected: `results: SmallVec<[ResultValue; 1]>` in Call variant.
  If not found, STOP — wave-1 must complete first.

- [ ] **Verify: Wave-0 builder template accepts Vec<ResultValue>**
  Run: `grep -n "cannot be a Vec" crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs`
  Expected: No matches.

- [ ] **Verify: Wave-0 `[...]` format syntax exists**
  Run: `grep -n "Optional" crates/kirin-derive-chumsky/src/format.rs`
  Expected: `FormatElement::Optional` variant exists.

- [ ] **Verify: Call struct accessors**
  Run: `grep -n "pub fn" crates/kirin-function/src/call.rs`
  Expected: `target()`, `args()`, `result()` — the last one will need renaming to `results()`.

## Regression Test

- [ ] **Write roundtrip test for multi-result call**
  In `tests/roundtrip/function.rs`:
  ```
  %a, %b = call @foo(%x) -> i32, f64
  ```

- [ ] **Write roundtrip test for void call (no results)**
  ```
  call @bar(%x)
  ```

- [ ] **Write roundtrip test for multi-value return**
  ```
  ret %a, %b
  ```

## Implementation Steps

- [ ] **Step 1: Update Call struct**
  In `call.rs`:
  ```rust
  #[chumsky(format = "$call {target}({args})[ -> {results:type}]")]
  pub struct Call<T: CompileTimeValue> {
      target: Symbol,
      args: Vec<SSAValue>,
      results: Vec<ResultValue>,
      #[kirin(default)]
      marker: std::marker::PhantomData<T>,
  }
  ```
  Update accessors: `result()` -> `results()` returning `&[ResultValue]`.

- [ ] **Step 2: Update Return struct**
  In `ret.rs` (lines 1-10):
  ```rust
  #[chumsky(format = "$ret[ {values}]")]
  pub struct Return<T: CompileTimeValue> {
      pub(crate) values: Vec<SSAValue>,
      #[kirin(default)]
      marker: std::marker::PhantomData<T>,
  }
  ```
  **Important:** Use `$ret[ {values}]` (with `[...]` optional section), NOT `$ret {values}`. The current codegen for `Vec<T>` uses `.separated_by(Comma).collect()` which requires at least one element. The `[...]` section makes the entire values list optional, mapping absence to `Vec::new()`. This enables void returns (`ret` with no arguments).
  - Text `ret` (no args) -> `values: vec![]`
  - Text `ret %a, %b` -> `values: vec![ssa_a, ssa_b]`

- [ ] **Step 3: Update Call interpret impl**
  In `interpret_impl.rs`, the Call interpret impl constructs `Continuation::Call`. Change:
  ```rust
  Ok(Continuation::Call {
      callee,
      stage: stage_id,
      args,
      results: self.results().iter().copied().collect(),  // Vec -> SmallVec
  })
  ```

- [ ] **Step 4: Update Return interpret impl**
  ```rust
  fn interpret<L>(&self, interp: &mut I) -> ... {
      let values: SmallVec<[I::Value; 1]> = self.values
          .iter()
          .map(|ssa| interp.read(*ssa))
          .collect::<Result<_, _>>()?;
      Ok(Continuation::Return(values))
  }
  ```

- [ ] **Step 5: Update Call unit tests**
  In `call.rs` tests: `res: TestSSAValue(100).into()` -> `results: vec![TestSSAValue(100).into()]`
  Update `has_one_result` -> verify `results().len() == 1`
  Update `result_accessor` -> `results_accessor`

- [ ] **Step 6: Update Return unit tests**
  In `ret.rs` tests: `value: TestSSAValue(0).into()` -> `values: vec![TestSSAValue(0).into()]`
  Update `has_one_argument` for Vec
  Update `value_field` test

- [ ] **Step 7: Update Lexical/Lifted interpret impls if needed**
  Check `interpret_impl.rs` for `Lexical::Call` and `Lexical::Return` delegation — these use `#[wraps]` so they auto-delegate, but verify the dispatch still compiles.

- [ ] **Step 8: Update roundtrip tests**
  In `tests/roundtrip/function.rs`:
  - Update existing tests for new format
  - Add multi-result call test
  - Add void call test
  - Add multi-value return test

- [ ] **Step 9: Run all tests**
  Run: `cargo nextest run -p kirin-function && cargo nextest run --workspace`
  Expected: All pass.

- [ ] **Step 10: Fix clippy**
  Run: `cargo clippy -p kirin-function`
  Expected: No warnings.

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations — fix root causes.
- Do NOT leave clippy warnings.
- Do NOT modify kirin-interpreter — done in wave-1.
- Do NOT modify kirin-scf — separate wave-2 plan.
- No unsafe code.

## Validation

**Final checks:**
```bash
cargo clippy -p kirin-function                # Expected: no warnings
cargo nextest run -p kirin-function           # Expected: all tests pass
cargo nextest run --workspace                  # Expected: no regressions
cargo test --doc -p kirin-function            # Expected: all doctests pass
```

## Success Criteria

1. `Call` supports 0-to-N results via `Vec<ResultValue>` with `[...]` optional format section.
2. `Return` carries `Vec<SSAValue>` for multi-value return.
3. Interpret impls produce correctly-shaped `Continuation::Call` and `Continuation::Return`.
4. Void call (`call @bar(%x)` with no results) roundtrips correctly.
5. Multi-result call roundtrips correctly.
6. Multi-value return roundtrips correctly.
7. All existing tests updated and passing.

**Is this a workaround or a real fix?**
Real fix. Full multi-result function dialect per design document.
