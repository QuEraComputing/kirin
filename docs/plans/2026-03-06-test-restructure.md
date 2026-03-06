# Test Restructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure tests into three test crates (`kirin-test-types`, `kirin-test-languages`, `kirin-test-utils`) and centralize roundtrip tests in workspace `tests/roundtrip/`.

**Architecture:** Create `kirin-test-types` for pure type definitions, consolidate shared test dialects into `kirin-test-languages`, keep helpers in `kirin-test-utils`, move all roundtrip tests to workspace `tests/roundtrip/<dialect>.rs`. Dialect crates keep only inline unit tests.

**Tech Stack:** Rust workspace, cargo features, insta snapshot tests.

---

### Task 1: Create `kirin-test-types` Crate

**Files:**
- Create: `crates/kirin-test-types/Cargo.toml`
- Create: `crates/kirin-test-types/src/lib.rs`
- Create: `crates/kirin-test-types/src/unit_type.rs`
- Create: `crates/kirin-test-types/src/simple_type.rs`
- Create: `crates/kirin-test-types/src/value.rs`
- Modify: `Cargo.toml` (workspace root — add member and dependency)

**Step 1: Create Cargo.toml for kirin-test-types**

```toml
[package]
name = "kirin-test-types"
version = "0.1.0"
edition = "2024"

[dependencies]
kirin-ir = { version = "0.1.0", path = "../kirin-ir", default-features = false }
kirin-chumsky = { workspace = true, optional = true }
kirin-lexer = { workspace = true, optional = true }
kirin-prettyless = { workspace = true, optional = true }

[features]
default = []
parser = ["kirin-chumsky", "kirin-lexer"]
pretty = ["kirin-prettyless"]
```

**Step 2: Move `UnitType` from `kirin-test-utils/src/unit_type.rs`**

Copy `crates/kirin-test-utils/src/unit_type.rs` to `crates/kirin-test-types/src/unit_type.rs`. Keep the original in kirin-test-utils temporarily (will re-export from new crate later).

**Step 3: Move `SimpleType` from `kirin-test-languages/src/simple_type.rs`**

Copy `crates/kirin-test-languages/src/simple_type.rs` to `crates/kirin-test-types/src/simple_type.rs`. Keep original temporarily.

**Step 4: Move `Value` from `kirin-test-languages/src/value.rs`**

Copy `crates/kirin-test-languages/src/value.rs` to `crates/kirin-test-types/src/value.rs`. Update `use crate::SimpleType` to `use crate::simple_type::SimpleType` or just `use crate::SimpleType` if re-exported from lib.rs.

**Step 5: Create `lib.rs`**

```rust
mod simple_type;
mod unit_type;
mod value;

pub use simple_type::SimpleType;
pub use unit_type::UnitType;
pub use value::Value;
```

**Step 6: Add to workspace**

In root `Cargo.toml`, add `"crates/kirin-test-types"` to `[workspace].members` and add:
```toml
kirin-test-types = { version = "0.1.0", path = "crates/kirin-test-types" }
```
to `[workspace.dependencies]`.

**Step 7: Build and verify**

Run: `cargo build -p kirin-test-types`
Expected: Clean build

Run: `cargo build -p kirin-test-types --features parser,pretty`
Expected: Clean build

**Step 8: Commit**

```bash
git add crates/kirin-test-types/ Cargo.toml Cargo.lock
git commit -m "feat: create kirin-test-types crate with UnitType, SimpleType, Value"
```

---

### Task 2: Wire `kirin-test-types` into Existing Crates

**Files:**
- Modify: `crates/kirin-test-utils/Cargo.toml`
- Modify: `crates/kirin-test-utils/src/lib.rs`
- Modify: `crates/kirin-test-utils/src/unit_type.rs` (replace with re-export)
- Modify: `crates/kirin-test-languages/Cargo.toml`
- Modify: `crates/kirin-test-languages/src/lib.rs`
- Delete or replace: `crates/kirin-test-languages/src/simple_type.rs`
- Delete or replace: `crates/kirin-test-languages/src/value.rs`

**Step 1: Update kirin-test-utils to depend on kirin-test-types**

In `crates/kirin-test-utils/Cargo.toml`, add:
```toml
kirin-test-types = { workspace = true }
```

Replace `crates/kirin-test-utils/src/unit_type.rs` contents with:
```rust
pub use kirin_test_types::UnitType;
```

Or just re-export from `lib.rs`:
```rust
pub use kirin_test_types::UnitType;
```
and remove the `unit_type` module entirely.

**Step 2: Update kirin-test-languages to depend on kirin-test-types**

In `crates/kirin-test-languages/Cargo.toml`, add:
```toml
kirin-test-types = { workspace = true, features = ["parser", "pretty"] }
```

Replace `crates/kirin-test-languages/src/simple_type.rs` with:
```rust
pub use kirin_test_types::SimpleType;
```

Replace `crates/kirin-test-languages/src/value.rs` with:
```rust
pub use kirin_test_types::Value;
```

Or simplify: delete those files and re-export from `lib.rs`:
```rust
pub use kirin_test_types::{SimpleType, Value};
```

**Step 3: Build full workspace**

Run: `cargo build --workspace`
Expected: Clean build. The re-exports preserve API compatibility.

**Step 4: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All 217 tests pass

**Step 5: Commit**

```bash
git add crates/kirin-test-utils/ crates/kirin-test-languages/ Cargo.toml Cargo.lock
git commit -m "refactor: wire kirin-test-types into test-utils and test-languages"
```

---

### Task 3: Consolidate Shared Test Dialects into `kirin-test-languages`

**Files:**
- Create: `crates/kirin-test-languages/src/numeric_language.rs`
- Create: `crates/kirin-test-languages/src/callable_language.rs`
- Create: `crates/kirin-test-languages/src/namespaced_language.rs`
- Modify: `crates/kirin-test-languages/Cargo.toml` (add features)
- Modify: `crates/kirin-test-languages/src/lib.rs`

**Step 1: Extract `NumericLanguage` from arith/bitwise tests**

Read `crates/kirin-arith/tests/arith.rs` for the `NumericLanguage` definition. Create `crates/kirin-test-languages/src/numeric_language.rs` with a generic version:

```rust
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::Return;
use kirin_ir::*;

/// Test dialect wrapping Arith + Constant + ControlFlow + Return.
/// Used for roundtrip testing of arithmetic and bitwise operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
#[wraps]
pub enum NumericLanguage {
    Arith(Arith<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}
```

Note: Check whether the bitwise tests also need `Bitwise` in the language. If so, create a separate `BitwiseNumericLanguage` or add a feature-gated variant.

Actually, looking at bitwise.rs, it defines its own `NumericLanguage` with `Bitwise` instead of `Arith`. So we need two dialects, or a single dialect with both. Check if having both `Arith` and `Bitwise` in one dialect causes ambiguity. If not, create a combined one. If yes, keep them separate:

- `ArithNumericLanguage` — Arith + Constant + ControlFlow + Return
- `BitwiseNumericLanguage` — Bitwise + Constant + ControlFlow + Return

**Step 2: Extract `CallableLanguage` from function tests**

Read `crates/kirin-function/tests/function_roundtrip.rs`. Create `crates/kirin-test-languages/src/callable_language.rs`:

```rust
use kirin_function::{Bind, Call, Return};
use kirin_ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum CallableLanguage {
    #[chumsky(format = "{body}")]
    Function { body: Region },
    #[wraps]
    Bind(Bind<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[kirin(terminator)]
    #[wraps]
    Return(Return<ArithType>),
}
```

**Step 3: Extract `NamespacedLanguage` and `BareLanguage` from namespace tests**

Read `tests/namespace_roundtrip.rs`. Create `crates/kirin-test-languages/src/namespaced_language.rs` with both language definitions.

**Step 4: Add feature flags and module declarations**

Add features to `crates/kirin-test-languages/Cargo.toml`:
```toml
numeric-language = ["kirin-arith", "kirin-cf", "kirin-constant", "kirin-function", ...]
callable-language = ["kirin-function", ...]
namespaced-language = ["kirin-arith", "kirin-cf", "kirin-function", ...]
```

Add to `lib.rs`:
```rust
#[cfg(feature = "numeric-language")]
mod numeric_language;
#[cfg(feature = "numeric-language")]
pub use numeric_language::*;
// ... etc
```

**Step 5: Build and verify**

Run: `cargo build -p kirin-test-languages --all-features`
Expected: Clean build

**Step 6: Commit**

```bash
git add crates/kirin-test-languages/
git commit -m "feat(test-languages): add NumericLanguage, CallableLanguage, NamespacedLanguage"
```

---

### Task 4: Create Workspace `tests/roundtrip/` Directory and `mod.rs`

**Files:**
- Create: `tests/roundtrip/mod.rs`
- Create: `tests/roundtrip.rs` (the test harness entry point)

**Step 1: Create the entry point**

Cargo integration tests need a `tests/roundtrip.rs` file that declares the module:

```rust
mod roundtrip;
```

Wait — actually, Cargo 2024 edition uses a different convention. In edition 2024, `tests/roundtrip/mod.rs` is NOT automatically a test. The convention is either:
- `tests/roundtrip.rs` as a single file, OR
- `tests/roundtrip/main.rs` as a directory test

Check the Rust edition 2024 test conventions. For a module directory structure, we need `tests/roundtrip/main.rs`:

```rust
mod arith;
mod bitwise;
mod cf;
mod cmp;
mod constant;
mod function;
mod namespace;
mod scf;
mod composite;
```

**Step 2: Create placeholder files**

Create empty files for each module:
- `tests/roundtrip/arith.rs`
- `tests/roundtrip/bitwise.rs`
- `tests/roundtrip/cf.rs`
- `tests/roundtrip/cmp.rs`
- `tests/roundtrip/constant.rs`
- `tests/roundtrip/function.rs`
- `tests/roundtrip/namespace.rs`
- `tests/roundtrip/scf.rs`
- `tests/roundtrip/composite.rs`

Each starts empty or with a single placeholder test:
```rust
#[test]
fn placeholder() {}
```

**Step 3: Update root Cargo.toml dev-dependencies**

Ensure the workspace `[dev-dependencies]` includes all dialect crates and test crates needed:
```toml
[dev-dependencies]
kirin-test-types = { workspace = true, features = ["parser", "pretty"] }
kirin-test-languages = { workspace = true, features = ["...all needed features..."] }
kirin-test-utils = { workspace = true, features = ["roundtrip"] }
kirin-arith = { workspace = true }
kirin-bitwise = { workspace = true }
kirin-cf = { workspace = true }
kirin-cmp = { workspace = true }
kirin-constant = { workspace = true }
kirin-function = { workspace = true }
kirin-scf = { workspace = true }
```

**Step 4: Build and run**

Run: `cargo nextest run --test roundtrip`
Expected: Placeholder tests pass

**Step 5: Commit**

```bash
git add tests/roundtrip/ tests/roundtrip.rs Cargo.toml Cargo.lock
git commit -m "feat: create tests/roundtrip/ directory structure"
```

Note: If edition 2024 uses `main.rs`, the entry file is `tests/roundtrip/main.rs` not `tests/roundtrip.rs`. Check the existing `tests/simple.rs` pattern to confirm.

---

### Task 5: Move Arith Roundtrip Tests

**Files:**
- Modify: `tests/roundtrip/arith.rs`
- Delete: `crates/kirin-arith/tests/arith.rs`
- Delete: `crates/kirin-arith/tests/function_roundtrip.rs`

**Step 1: Move statement roundtrip tests**

Copy the test functions from `crates/kirin-arith/tests/arith.rs` into `tests/roundtrip/arith.rs`. Replace the inline `NumericLanguage` with the one from `kirin_test_languages`. Replace the inline `assert_roundtrip()` with `kirin_test_utils::roundtrip::assert_statement_roundtrip()`.

Update imports:
```rust
use kirin_test_languages::NumericLanguage;  // or ArithNumericLanguage
use kirin_test_utils::roundtrip;
use kirin_arith::ArithType;
```

**Step 2: Move pipeline roundtrip tests**

Copy tests from `crates/kirin-arith/tests/function_roundtrip.rs`. Replace inline `ArithmeticFunctionLanguage` with appropriate shared dialect. If no shared dialect covers the function+region pattern, define a minimal one in the `arith.rs` test file or add it to `kirin-test-languages`.

Note: The `ArithmeticFunctionLanguage` has `Function { body: Region }` which is NOT a `#[wraps]` variant — it's inlined because of E0275. This means it can't easily be shared. Keep it inline in the test file if necessary.

**Step 3: Delete old test files**

```bash
rm crates/kirin-arith/tests/arith.rs
rm crates/kirin-arith/tests/function_roundtrip.rs
rmdir crates/kirin-arith/tests/  # if empty
```

Also remove any `[[test]]` sections from `crates/kirin-arith/Cargo.toml` if present, and remove `kirin-test-utils` from its `[dev-dependencies]` if no longer needed.

**Step 4: Build and run all tests**

Run: `cargo nextest run --workspace`
Expected: Same number of tests, all pass. Tests moved, not added/removed.

**Step 5: Commit**

```bash
git add tests/roundtrip/arith.rs
git rm crates/kirin-arith/tests/arith.rs crates/kirin-arith/tests/function_roundtrip.rs
git commit -m "refactor: move arith roundtrip tests to workspace tests/roundtrip/"
```

---

### Task 6: Move Bitwise Roundtrip Tests

Same pattern as Task 5. Move from `crates/kirin-bitwise/tests/` to `tests/roundtrip/bitwise.rs`.

**Step 1:** Copy tests, replace inline `NumericLanguage` with shared `BitwiseNumericLanguage` from `kirin-test-languages`.

**Step 2:** Copy pipeline roundtrip from `function_roundtrip.rs`.

**Step 3:** Delete old files, update Cargo.toml.

**Step 4:** `cargo nextest run --workspace` — all pass.

**Step 5:** Commit: `"refactor: move bitwise roundtrip tests to workspace tests/roundtrip/"`

---

### Task 7: Move Function Roundtrip Tests

Move from `crates/kirin-function/tests/` to `tests/roundtrip/function.rs`.

**Step 1:** Copy `function_roundtrip.rs` tests. Replace inline `CallableLanguage` with shared one from `kirin-test-languages`.

**Step 2:** Copy `lambda_print.rs` tests. The `LambdaLanguage` has inlined fields (E0275 workaround) — keep it inline in the test file.

**Step 3:** Delete old files.

**Step 4:** `cargo nextest run --workspace` — all pass.

**Step 5:** Commit: `"refactor: move function roundtrip tests to workspace tests/roundtrip/"`

---

### Task 8: Move Namespace Roundtrip Tests

Move from `tests/namespace_roundtrip.rs` to `tests/roundtrip/namespace.rs`.

**Step 1:** Move the file. Replace inline `NamespacedLanguage` and `BareLanguage` with shared ones from `kirin-test-languages` (if they were consolidated in Task 3).

**Step 2:** Delete `tests/namespace_roundtrip.rs`.

**Step 3:** `cargo nextest run --workspace` — all pass.

**Step 4:** Commit: `"refactor: move namespace roundtrip tests to tests/roundtrip/"`

---

### Task 9: Add CMP and Constant Roundtrip Tests

These dialects currently have NO roundtrip tests (gap identified in the test audit). Add them.

**Step 1: Write CMP roundtrip tests in `tests/roundtrip/cmp.rs`**

Test each comparison operation: `eq`, `ne`, `lt`, `le`, `gt`, `ge`. Follow the same pattern as arith — define statements, roundtrip parse/print.

```rust
use kirin_test_utils::roundtrip;

// Define or use a shared CmpLanguage that wraps Cmp + Constant + Return
// Or use a combined dialect from kirin-test-languages

#[test]
fn test_eq_roundtrip() {
    roundtrip::assert_statement_roundtrip::<CmpLanguage>(
        "%res = eq %a, %b -> i64",
        // ... type context
    );
}
// ... same for ne, lt, le, gt, ge
```

**Step 2: Write Constant roundtrip test in `tests/roundtrip/constant.rs`**

```rust
#[test]
fn test_constant_roundtrip() {
    roundtrip::assert_statement_roundtrip::<ConstantLanguage>(
        "%x = constant 42 -> i64",
    );
}
```

**Step 3: Run and verify**

Run: `cargo nextest run --test roundtrip`
Expected: New tests pass

**Step 4: Commit**

```bash
git commit -m "test: add cmp and constant roundtrip tests"
```

---

### Task 10: Add CF and SCF Roundtrip Tests

**Step 1: Write CF tests in `tests/roundtrip/cf.rs`**

Test `br` and `cond_br` roundtrips. These are terminators and need a language that includes them.

**Step 2: Write SCF tests in `tests/roundtrip/scf.rs`**

Test `if`, `for`, `yield` roundtrips. These contain `Block` fields.

**Step 3: Run and verify**

**Step 4: Commit: `"test: add cf and scf roundtrip tests"`**

---

### Task 11: Consolidate `tests/simple.rs` into `tests/roundtrip/composite.rs`

**Step 1:** Move the roundtrip tests from `tests/simple.rs` to `tests/roundtrip/composite.rs`. These test multi-dialect composition with `SimpleLanguage`.

**Step 2:** Keep `tests/simple.rs` for the `test_block` IR-building test (or move it to a `tests/ir_construction.rs`).

**Step 3:** Run and verify.

**Step 4:** Commit: `"refactor: move composite roundtrip tests from simple.rs to roundtrip/"`**

---

### Task 12: Fix SimpleLanguage Format Strings

The `SimpleLanguage` format strings in `kirin-test-languages/src/simple_language.rs` were missed during the `{.keyword}` migration:

```rust
// Current (bare keywords):
#[chumsky(format = "{2:name} = add {0}, {1} -> {2:type}")]
#[chumsky(format = "{1:name} = constant {0} -> {1:type}")]
#[chumsky(format = "return {0}")]
#[chumsky(format = "{1:name} = function {0}")]

// Should be:
#[chumsky(format = "{2:name} = {.add} {0}, {1} -> {2:type}")]
#[chumsky(format = "{1:name} = {.constant} {0} -> {1:type}")]
#[chumsky(format = "{.return} {0}")]
#[chumsky(format = "{1:name} = {.function} {0}")]
```

**Step 1:** Apply migrations.

**Step 2:** Update any snapshot tests that reference these strings.

**Step 3:** Run: `cargo nextest run --workspace`

**Step 4:** Commit: `"fix: migrate SimpleLanguage format strings to {.keyword} syntax"`

---

### Task 13: Add Codegen Snapshot Tests for `{.keyword}`

**Files:**
- Modify: `crates/kirin-derive-chumsky/src/codegen/parser/chain.rs` (add test module)
- Modify: `crates/kirin-derive-chumsky/src/codegen/pretty_print/statement.rs` (add test module)

**Step 1: Write parser codegen snapshot test**

In `chain.rs` or a nearby test file, add a test that:
1. Creates a mock `Format` with `FormatElement::Keyword("add")`
2. Calls `build_parser_chain` or `keyword_parser`
3. Snapshots the generated `TokenStream` via `insta::assert_snapshot!(rustfmt_token_stream(&output))`

This may require extracting `keyword_parser` to be testable (it currently takes `&self`). If testing the method directly is hard, create a minimal proc-macro integration test that derives a type with `{.add}` and snapshots the expansion.

**Step 2: Write pretty-print codegen snapshot test**

Similar to Step 1 but for the pretty-print keyword codegen.

**Step 3: Write wrapper namespace extension snapshot test**

Test that a `#[wraps]` variant with `#[chumsky(format = "arith")]` generates the namespace push + delegation code.

**Step 4: Run and accept snapshots**

Run: `cargo nextest run -p kirin-derive-chumsky`
Run: `cargo insta review`
Expected: Accept new snapshots

**Step 5: Commit**

```bash
git commit -m "test(derive-chumsky): add codegen snapshots for keyword and namespace codegen"
```

---

### Task 14: Clean Up Dialect Crate Dev-Dependencies

After all roundtrip tests are moved to workspace `tests/`, dialect crates no longer need test-related dev-dependencies.

**Step 1: Check each dialect crate Cargo.toml**

For `kirin-arith`, `kirin-bitwise`, `kirin-function`, `kirin-cf`, `kirin-cmp`, `kirin-constant`, `kirin-scf`:
- Remove `[dev-dependencies]` entries for `kirin-test-utils`, `kirin-test-languages`, `insta`
- Remove empty `tests/` directories
- Keep any `#[cfg(test)]` inline tests

**Step 2: Build and run**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: Same test count, all pass

**Step 3: Commit**

```bash
git commit -m "chore: clean up dialect crate dev-dependencies after test migration"
```

---

### Task 15: Final Verification and Documentation

**Step 1: Full workspace build**

Run: `cargo build --workspace`

**Step 2: Full test suite**

Run: `cargo nextest run --workspace`

**Step 3: Doc tests**

Run: `cargo test --doc --workspace`

**Step 4: Format**

Run: `cargo fmt --all`

**Step 5: Verify example still works**

Run: `cargo run --example simple`
Expected: `roundtrip OK`

**Step 6: Update AGENTS.md test conventions section**

Add a "Test Conventions" section to `AGENTS.md`:

```markdown
## Test Conventions

- **Roundtrip tests** (parse → emit → print → compare) go in workspace `tests/roundtrip/<dialect>.rs`
- **Unit tests** for internal logic go inline in the crate (`#[cfg(test)]`)
- **Codegen snapshot tests** go inline in `kirin-derive-chumsky`
- **IR rendering snapshots** go inline in `kirin-prettyless`
- **New test types** (type lattices, values) go in `kirin-test-types`
- **New test dialects** (language enums, stage enums) go in `kirin-test-languages`
- **New test helpers** (roundtrip, parse, fixture builders) go in `kirin-test-utils`
```

**Step 7: Commit**

```bash
git commit -m "docs: add test conventions to AGENTS.md"
```
