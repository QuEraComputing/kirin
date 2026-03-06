---
name: insta-snapshot-testing
description: Use when writing, updating, or reviewing snapshot tests with insta in the kirin project
---

# Insta Snapshot Testing

## Overview

Insta captures output as snapshot files. Snapshot files are **generated artifacts** — never manually create, edit, or delete `.snap` files. All snapshot management goes through `cargo insta`.

## Quick Reference

| Action | Command |
|--------|---------|
| Run tests (creates `.snap.new` for changes) | `cargo nextest run -p <crate>` |
| Review pending snapshots interactively | `cargo insta review` |
| Accept all pending snapshots | `cargo insta accept` |
| Reject all pending snapshots | `cargo insta reject` |
| Run + review in one step | `cargo insta test -p <crate> --review` |
| Test single snapshot test | `cargo nextest run -p <crate> -E 'test(test_name)'` |

## Assertion Macros

```rust
// Auto-named (module path + test function → .snap filename)
insta::assert_snapshot!(string_value);

// Debug format (for structs implementing Debug)
insta::assert_debug_snapshot!(value);

// Explicit name (when multiple snapshots per test)
insta::assert_snapshot!("snapshot_name", string_value);

// Inline (small values, no .snap file created)
insta::assert_snapshot!(value, @"expected text");
```

## Snapshot File Locations

Snapshots live in `snapshots/` directories **adjacent to the test source file**:

```
src/format.rs          → src/snapshots/crate__format__tests__test_name.snap
tests/roundtrip.rs     → tests/snapshots/roundtrip__test_name.snap
```

## Workflow

1. **Write test** with `insta::assert_snapshot!(output)`
2. **Run test** — first run creates `.snap.new` (pending) file, test "fails"
3. **Review** with `cargo insta review` — interactively accept/reject each change
4. **Commit** the `.snap` files alongside test code

## Key Rules

- **Never `rm` snapshot files** — they are generated. Use `cargo insta reject` or just delete the test.
- **Never manually edit `.snap` files** — always regenerate by running the test.
- **Always `cargo insta review`** after test changes — don't blindly accept with `cargo insta accept`.
- **Commit `.snap` files** — they are source-controlled test expectations.
- **Inline snapshots** (`@"..."`) update in-place in source when accepted.

## Snap File Format

```
---
source: crates/kirin-derive-chumsky/src/format.rs
expression: format
---
<snapshot content here>
```

The `source` and `expression` fields are metadata. Content below the second `---` is the expected output.

## Common Patterns in Kirin

**Codegen snapshots** (derive macro output):
```rust
let output = generate(&ir).unwrap();
let formatted = rustfmt_token_stream(&output);
insta::assert_snapshot!(formatted);
```

**Pretty-print snapshots** (IR rendering):
```rust
let buf = pipeline.sprint::<L>();
insta::assert_snapshot!(buf);
```

**Format parser snapshots** (parsed format strings):
```rust
let format = Format::parse(input, span).unwrap();
insta::assert_debug_snapshot!(format);
```

## When Snapshots Change Unexpectedly

If tests fail with snapshot mismatches after a code change:
1. Run `cargo insta review` to see the diff
2. Determine if the change is intentional (accept) or a regression (fix code)
3. Accept intentional changes, fix regressions
