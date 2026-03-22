# CF Roundtrip Mismatch Fix

**Finding(s):** #1 CF Roundtrip Mismatch
**Wave:** 1
**Agent role:** Implementer
**Estimated effort:** moderate

---

## Issue

`Successor::Display` (in `kirin-ir/src/node/block.rs:45-48`) outputs raw arena IDs (`^0`, `^1`), while block headers in the pretty printer resolve names through the symbol table (`^entry`, `^exit`). This creates a roundtrip mismatch: parsing `br ^exit(%x)` then printing produces `br ^0(%x)`.

CF tests currently cannot use full roundtrip assertions. They are parse-only tests with structural assertions (block count, terminator presence).

**Full finding text from implementation-notes.md:**
> `Successor::Display` (in `kirin-ir/src/node/block.rs:45-48`) outputs raw arena IDs (`^0`, `^1`), while block headers in the pretty printer resolve names through the symbol table (`^entry`, `^exit`). This creates a roundtrip mismatch: parsing `br ^exit(%x)` then printing produces `br ^0(%x)`.
>
> **Fix required:** `Successor`'s pretty printing needs access to the stage's symbol table to resolve IDs back to names. This is a deeper change — `Display` doesn't carry context, so it would need to go through `PrettyPrint` with an `IRRenderCtx` instead.

**Crate(s):** kirin-prettyless (primary), tests/roundtrip/cf.rs
**File(s):**
- `crates/kirin-prettyless/src/impls.rs:77` — `impl PrettyPrintViaDisplay for Successor {}`
- `crates/kirin-prettyless/src/document/ir_render.rs:19-31` — `resolve_caret_name()` method
- `crates/kirin-prettyless/src/tests/impls.rs:262-277` — Successor pretty print test
- `tests/roundtrip/cf.rs` — CF parse-only tests to convert to full roundtrip
- `crates/kirin-ir/src/node/block.rs:45-48` — `Successor::Display` (NOT modified)
**Confidence:** confirmed

## Guiding Principles

- "Block vs Region: A `Block` is a single linear sequence of statements with an optional terminator. A `Region` is a container for multiple blocks (`LinkedList<Block>`)." — Understanding block/region distinction is important for how Successor targets blocks.
- "Roundtrip tests (parse → emit → print → compare) go in workspace `tests/roundtrip/<dialect>.rs`" — CF roundtrip tests live in `tests/roundtrip/cf.rs`.
- "No unsafe code. All implementations MUST use safe Rust." — Standard safety constraint.
- "`mod.rs` should stay lean: only module declarations (`mod`), re-exports (`pub use`), and prelude definitions. Move substantial logic into sibling files within the same directory." — Keep impls.rs changes focused.

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-prettyless/src/impls.rs` | modify | Replace `impl PrettyPrintViaDisplay for Successor {}` with manual `impl PrettyPrint for Successor` that resolves block names via symbol table |
| `crates/kirin-prettyless/src/tests/impls.rs` | modify | Update Successor test to verify name resolution (not just `^` prefix) |
| `tests/roundtrip/cf.rs` | modify | Convert parse-only tests to full roundtrip assertions |

**Files explicitly out of scope:**
- `crates/kirin-ir/src/node/block.rs` — `Successor::Display` stays as-is (raw IDs). The fix is in PrettyPrint, not Display.
- `crates/kirin-cf/src/lib.rs` — CF dialect definitions unchanged. The format strings use `{target}` etc. which delegate to the field's PrettyPrint impl.
- `crates/kirin-prettyless/src/document/ir_render.rs` — `resolve_caret_name()` already exists and works correctly. No changes needed.

## Verify Before Implementing

- [ ] **Verify: `resolve_caret_name` exists and works as expected**
  Run: `cargo nextest run -p kirin-prettyless -E 'test(test_pretty_print_successor)'`
  Expected: PASS. The current test asserts `buf.starts_with("^")` which passes with raw IDs.
  If this fails, STOP and report — the test infrastructure may have changed.

- [ ] **Verify: `Successor` has access to `Block` via `target()` method**
  Check that `Successor::target(self) -> Block` exists at `crates/kirin-ir/src/node/block.rs:29-31`.
  Expected: The method exists and returns `Block(self.0)`.

- [ ] **Verify: `BlockInfo` has a `name: Option<Symbol>` field**
  Check `crates/kirin-ir/src/node/block.rs:55` — `BlockInfo` should have `pub name: Option<Symbol>`.
  Expected: Field exists. This is what `resolve_caret_name` uses to look up the symbolic name.

- [ ] **Verify: `Block` implements `GetInfo` to access `BlockInfo`**
  Run: `grep -n "impl.*GetInfo.*Block" crates/kirin-ir/src/`
  Expected: `Block` implements `GetInfo<L, Info = BlockInfo<L>>` at `kirin-ir/src/node/block.rs:97`, enabling `block.expect_info(stage)`. (Verified during plan review.)

## Implementation Steps

- [ ] **Step 1: Replace `PrettyPrintViaDisplay` with manual `PrettyPrint` for `Successor`**
  In `crates/kirin-prettyless/src/impls.rs`, remove line 77:
  ```rust
  impl PrettyPrintViaDisplay for Successor {}
  ```
  Replace with a manual `PrettyPrint` impl that:
  1. Calls `self.target()` to get the `Block`
  2. Calls `block.expect_info(stage)` to get `BlockInfo`
  3. Uses `resolve_caret_name(block_info.name, self.target())` (from the `Document`) to resolve the name

  The implementation pattern follows `Symbol`'s PrettyPrint impl (lines 79-96 in the same file) which also does symbol table resolution. The key difference is that `Successor` needs to go through `Block` → `BlockInfo` → `name: Option<Symbol>`, then use `resolve_caret_name()`.

  ```rust
  impl PrettyPrint for Successor {
      fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
          &self,
          doc: &'a Document<'a, L>,
          _namespace: &[&str],
      ) -> ArenaDoc<'a>
      where
          L::Type: std::fmt::Display,
      {
          let block = self.target();
          let block_info = block.expect_info(doc.stage());
          doc.text(doc.resolve_caret_name(block_info.name, block))
      }
  }
  ```

  **Visibility notes (verified):**
  - `resolve_caret_name` is currently a private method on `Document<'a, L>` (in `ir_render.rs:23`, no `pub` modifier). It needs to be made `pub(crate)` in Step 2 since `impls.rs` is in the same crate.
  - `doc.stage()` is already `pub` (defined at `document/builder.rs:107` as `pub fn stage(&self) -> &'a StageInfo<L>`) — no changes needed.

- [ ] **Step 2: Make `resolve_caret_name` accessible from `impls.rs`**
  In `crates/kirin-prettyless/src/document/ir_render.rs`, change:
  ```rust
  fn resolve_caret_name(...)
  ```
  to:
  ```rust
  pub(crate) fn resolve_caret_name(...)
  ```
  Also verify that `doc.stage()` returns a reference to the stage. If the `stage` field is `pub(crate)`, you can use `doc.stage` directly. If there's a `stage()` getter, use that.

  Run: `cargo check -p kirin-prettyless`
  Expected: Compiles without errors.

- [ ] **Step 3: Update the Successor pretty print test**
  In `crates/kirin-prettyless/src/tests/impls.rs`, the test at line 264 (`test_pretty_print_successor`) currently asserts only `buf.starts_with("^")`. Update it to:
  1. Give the block a name via the symbol table (like the Symbol test does)
  2. Assert the full resolved name appears (e.g., `^target`)

  Also add a second test case for unnamed blocks that verifies fallback to raw ID format.

  Run: `cargo nextest run -p kirin-prettyless -E 'test(test_pretty_print_successor)'`
  Expected: PASS with the new assertions.

- [ ] **Step 4: Convert CF roundtrip tests to full roundtrip**
  In `tests/roundtrip/cf.rs`, replace the `parse_cf_program` helper and structural assertions with the standard roundtrip pattern from other test files (e.g., `tests/roundtrip/arith.rs`).

  The roundtrip pattern is: parse input → print → compare printed output to expected.

  For each test (`test_branch_parse`, `test_conditional_branch_parse`, `test_branch_with_multiple_args`, `test_diamond_control_flow`):
  - Parse the input
  - Print via `pipeline.sprint()`
  - Assert the printed output contains the symbolic block names (`^entry`, `^exit`, etc.) in the branch instructions, not just in block headers
  - Ideally, re-parse the printed output to verify full roundtrip

  Run: `cargo nextest run --test roundtrip`
  Expected: All CF tests pass with full roundtrip assertions. (The roundtrip tests are in the top-level `kirin` crate's `tests/roundtrip/main.rs` binary.)

- [ ] **Step 5: Run full validation**
  Run:
  ```bash
  cargo clippy -p kirin-prettyless
  cargo nextest run -p kirin-prettyless
  cargo nextest run --workspace
  cargo test --doc --workspace
  ```
  Expected: All pass, no warnings.

## Must Not Do

- Do NOT modify `Successor::Display` in `kirin-ir/src/node/block.rs` — Display should remain raw IDs for debugging. Only PrettyPrint uses the symbol table.
- Do NOT introduce `#[allow(...)]` annotations to suppress warnings — fix the underlying cause.
- Do NOT leave clippy warnings. Run `cargo clippy -p kirin-prettyless` before reporting completion and fix all warnings.
- Do NOT change CF dialect definitions in `kirin-cf/src/lib.rs` — the format strings (`{target}`, `{true_target}`, `{false_target}`) already delegate to PrettyPrint, which is what we're fixing.
- Do NOT add public API surface to `kirin-prettyless` beyond what's needed. `resolve_caret_name` should stay `pub(crate)`, not `pub`.

## Validation

**Per-step checks:**
- After step 1+2: `cargo check -p kirin-prettyless` — Expected: compiles
- After step 3: `cargo nextest run -p kirin-prettyless -E 'test(test_pretty_print_successor)'` — Expected: PASS
- After step 4: `cargo nextest run --test roundtrip` — Expected: all roundtrip tests pass including CF

**Final checks:**
```bash
cargo clippy -p kirin-prettyless              # Expected: no warnings
cargo nextest run -p kirin-prettyless         # Expected: all tests pass
cargo nextest run --workspace                 # Expected: no regressions
cargo test --doc --workspace                  # Expected: all doctests pass
```

**Snapshot tests:** Possibly — the existing Successor snapshot test may change. Run `cargo insta test -p kirin-prettyless` and report changes, do NOT auto-accept.

## Success Criteria

1. `Successor` fields in printed CF operations (`br`, `cond_br`) show symbolic block names (e.g., `^exit`) matching block headers, not raw IDs (`^0`).
2. CF roundtrip tests in `tests/roundtrip/cf.rs` use full roundtrip assertions (parse → print → compare or parse → print → re-parse).
3. All existing tests pass — no regressions in any dialect's pretty printing.
4. `Successor::Display` is unchanged — only `PrettyPrint` is affected.

**Is this a workaround or a real fix?**
This is the real fix. The root cause is that `Successor` used `PrettyPrintViaDisplay` which delegates to `Display::fmt`, which has no access to the symbol table. The fix replaces this with a proper `PrettyPrint` impl that resolves names through the stage's symbol table, matching how block headers already work.
