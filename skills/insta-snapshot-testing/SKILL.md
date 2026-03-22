---
name: insta-snapshot-testing
description: Use when writing new snapshot tests, updating snapshots after code changes, reviewing pending `.snap.new` files, cleaning up orphaned snapshots, or troubleshooting cargo insta failures. Triggers on insta assertions, snapshot mismatches, `.snap` file management, or CI snapshot rejection.
user-invocable: false
---

# Insta Snapshot Testing

**Announce at start:** State which skill is being used so the user knows what process is driving behavior.

## Overview

Insta captures output as `.snap` files. These are **generated artifacts** — all management goes through `cargo insta`. Never manually create, edit, move, or delete `.snap` or `.snap.new` files.

## Quick Reference

| Action | Command |
|--------|---------|
| Run tests (creates `.snap.new` for mismatches) | `cargo nextest run -p <crate>` |
| Review pending snapshots interactively | `cargo insta review` |
| Accept all pending | `cargo insta accept` |
| Reject all pending | `cargo insta reject` |
| Run + review in one step | `cargo insta test -p <crate> --review` |
| Run + accept in one step | `cargo insta test -p <crate> --accept` |
| Accept only brand-new snapshots | `cargo insta test --accept-unseen` |
| Delete orphaned snapshots | `cargo insta test --unreferenced delete` |
| Force-update all snapshots | `cargo insta test --force-update-snapshots` |
| Target single snapshot | `cargo insta review --snapshot path/to/file.snap` |

## Assertion Macros

```rust
insta::assert_snapshot!(string_value);                    // auto-named from test fn
insta::assert_snapshot!("explicit_name", string_value);   // explicit name
insta::assert_debug_snapshot!(value);                     // Debug format
insta::assert_snapshot!(value, @"expected inline text");  // inline (no .snap file)
```

Auto-naming: test function name with `test_` prefix stripped. Multiple snapshots per test get `-2`, `-3` suffixes. Inline snapshots update in-place in `.rs` source when accepted.

## Snapshot File Locations

Snapshots live in `snapshots/` adjacent to the test source. Naming: `<crate>__<module_path>__<test_name>.snap`.

```
src/format.rs              → src/snapshots/<crate>__format__tests__test_name.snap
src/codegen/parser/gen.rs  → src/codegen/parser/snapshots/...__gen__tests__test_name.snap
tests/stack_interp.rs      → tests/snapshots/stack_interp__test_name.snap
```

## Snapshot File Format

```
---
source: <crate>/src/<file>.rs
expression: <expression>
---
<snapshot content here>
```

Pending changes are written as `.snap.new` files alongside existing `.snap` files.

## Workflows

### New snapshot test
1. Write test with `insta::assert_snapshot!(output)`
2. Run test — creates `.snap.new`, test "fails" (expected)
3. `cargo insta review` — accept if correct
4. Commit `.snap` file with test code

### Update after code change
1. Run tests — mismatches create `.snap.new`
2. `cargo insta review` — accept intentional changes, fix regressions
3. Commit updated `.snap` files

### Delete a snapshot test
1. Delete the test function
2. `cargo insta test --unreferenced delete` — removes orphaned `.snap` files
3. Commit

### Move/rename test files
1. New location gets new `snapshots/` dir automatically
2. `cargo insta test --unreferenced delete` — cleans old location
3. Commit new snapshots + deletions

`--unreferenced` modes: `ignore`, `warn`, `reject` (CI), `delete`, `auto` (delete locally, reject in CI).

## Key Rules

- **Never manually edit/rm/mv `.snap` files** — use `cargo insta` commands
- **Review before accepting** — `cargo insta review` shows diffs; don't blindly accept
- **Commit `.snap` files** — they are source-controlled expectations
- CI auto-detects (`CI` env var) and sets `INSTA_UPDATE=no` to prevent overwrites

## Common Patterns in Kirin

```rust
// Codegen snapshots (derive macro output)
let tokens = generator.generate(&ir_input);
insta::assert_snapshot!(rustfmt(tokens.to_string()));

// Pretty-print snapshots (IR rendering)
insta::assert_snapshot!(pipeline.sprint::<L>());

// Format parser snapshots
insta::assert_debug_snapshot!(Format::parse(input, None).unwrap());
```

## Troubleshooting

| Problem | Fix |
|---------|-----|
| Tests fail but output looks correct | `cargo insta review` and accept |
| Stale `.snap.new` files | `cargo insta reject` |
| Orphaned snapshots after moving tests | `cargo insta test --unreferenced delete` |
| Non-deterministic output (HashMaps) | Use `with_settings! { sort_maps => true, { ... } }` |
| Wrong auto-name for multiple snapshots | Use explicit: `assert_snapshot!("name", value)` |
