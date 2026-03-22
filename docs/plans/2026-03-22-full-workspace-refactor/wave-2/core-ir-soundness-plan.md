# Core IR Soundness Fixes

**Finding(s):** P1-4
**Wave:** 2
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

### P1-4: DenseHint::insert_or_combine silently drops value when ID is out of range

**Crate:** kirin-ir | **File:** `crates/kirin-ir/src/arena/hint/dense.rs:46-59`

`insert_or_combine` calls `self.data.get_mut(id.into().raw())`. If the index is beyond the current vec length, `get_mut` returns `None` and the value is silently dropped. In contrast, `insert()` on the same type dynamically resizes with `resize_with`. This inconsistency means callers using `insert_or_combine` may lose data without any indication.

**User decision:** Write tests triggering the bug BEFORE fixing.

**Crate(s):** kirin-ir
**File(s):** `crates/kirin-ir/src/arena/hint/dense.rs:46-59`
**Confidence:** likely

## Guiding Principles

- "No unsafe code." All implementations MUST use safe Rust.
- "less standalone function is better" -- prefer methods on existing types.
- IR Design: The arena hint system provides metadata annotation for arena items. `DenseHint` stores metadata in a `Vec<Option<T>>` indexed by arena item IDs.

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-ir/src/arena/hint/dense.rs` | modify | Add resize logic to `insert_or_combine` matching `insert()` |
| `crates/kirin-ir/src/arena/hint/dense.rs` | modify | Add regression test inline |

**Files explicitly out of scope:**
- `crates/kirin-ir/src/arena/hint/sparse.rs` -- different data structure, not affected
- `crates/kirin-ir/src/detach.rs` -- covered by LHF items (P1-1, P1-2)
- `crates/kirin-ir/src/arena/gc.rs` -- covered by LHF (P1-3)

## Verify Before Implementing

- [ ] **Verify: `insert_or_combine` does NOT resize**
  Run: Read `crates/kirin-ir/src/arena/hint/dense.rs` lines 46-59
  Expected: No `resize_with` or capacity expansion in `insert_or_combine`

- [ ] **Verify: `insert` DOES resize**
  Run: Read `crates/kirin-ir/src/arena/hint/dense.rs` lines 40-45
  Expected: Has `resize_with` for out-of-range IDs

- [ ] **Verify: existing tests pass**
  Run: `cargo nextest run -p kirin-ir`
  Expected: All tests pass

## Regression Test (P0/P1 findings)

- [ ] **Write regression test for P1-4: insert_or_combine drops value on out-of-range ID**
  Create a test that:
  1. Creates a `DenseHint` from a small arena (e.g., 2 items)
  2. Calls `insert_or_combine` with an ID beyond the current length (e.g., index 5)
  3. Attempts to `get` the value back
  4. Asserts the value is present (currently fails -- value is silently dropped)

  Test file: Inline `#[cfg(test)]` in `crates/kirin-ir/src/arena/hint/dense.rs`

- [ ] **Run the test -- confirm it fails (demonstrates the issue)**
  Run: `cargo nextest run -p kirin-ir -E 'test(insert_or_combine)'`
  Expected: FAIL -- get returns None because the value was silently dropped

## Implementation Steps

- [ ] **Step 1: Add resize logic to `insert_or_combine`**
  In `crates/kirin-ir/src/arena/hint/dense.rs`, add the same resize logic from `insert()` at the beginning of `insert_or_combine`:
  ```rust
  let idx = id.into().raw();
  if idx >= self.data.len() {
      self.data.resize_with(idx + 1, || None);
  }
  ```
  Then use `self.data[idx]` or `self.data.get_mut(idx)` (now guaranteed in-range) for the combine logic.

- [ ] **Step 2: Run regression test -- confirm it passes**
  Run: `cargo nextest run -p kirin-ir -E 'test(insert_or_combine)'`
  Expected: PASS

- [ ] **Step 3: Run full crate tests**
  Run: `cargo nextest run -p kirin-ir`
  Expected: All tests pass

- [ ] **Step 4: Run clippy**
  Run: `cargo clippy -p kirin-ir`
  Expected: No warnings

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations to suppress warnings.
- Do NOT change the `insert_or_combine` function signature -- keep the same API.
- Do NOT modify `insert()` -- it already works correctly.
- No unsafe code.

## Validation

**Per-step checks:**
- After step 1: `cargo check -p kirin-ir` -- Expected: compiles
- After step 2: `cargo nextest run -p kirin-ir -E 'test(insert_or_combine)'` -- Expected: PASS

**Final checks:**
```bash
cargo clippy -p kirin-ir                     # Expected: no warnings
cargo nextest run -p kirin-ir                # Expected: all tests pass
cargo nextest run --workspace                # Expected: no regressions
cargo test --doc -p kirin-ir                 # Expected: all doctests pass
```

**Snapshot tests:** No snapshot tests expected in this module.

## Success Criteria

1. `insert_or_combine` correctly resizes the internal vector when the ID is out of range, matching the behavior of `insert()`.
2. The regression test demonstrates the fix: a value inserted via `insert_or_combine` with an out-of-range ID is retrievable via `get()`.
3. No regressions in any workspace test.

**Is this a workaround or a real fix?**
This is the real fix. The inconsistency between `insert` (resizes) and `insert_or_combine` (drops) is a bug. Making both resize consistently is the correct behavior.
