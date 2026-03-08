# Toy Language End-to-End Example

**Date:** 2026-03-08
**Status:** Approved

## Goal

Build a toy language binary that reads `.kirin` text files, parses them into IR, and interprets functions — proving out the full text → parse → interpret → result pipeline.

## Project Structure

```
example/toy-lang/
├── Cargo.toml              # binary crate (workspace member)
├── src/
│   ├── main.rs             # CLI entry (clap subcommands: parse, run)
│   ├── language.rs          # HighLevel + LowLevel language enums
│   ├── stage.rs             # Stage enum (Source, Lowered)
│   └── interpret.rs         # StackInterpreter wiring
├── programs/
│   ├── add.kirin
│   ├── factorial.kirin
│   └── branching.kirin
└── tests/
    └── e2e.rs               # integration tests via assert_cmd
```

## Language Definitions

### HighLevel (source stage)

Structured control flow + lexical lambdas:

- `Arith<ArithType>` — add, sub, mul, div, rem, neg
- `Cmp<ArithType>` — eq, ne, lt, le, gt, ge
- `Bitwise<ArithType>` — and, or, xor, not, shl, shr
- `Constant<ArithValue, ArithType>` — literals
- `Return<ArithType>` — terminator
- Inline `If`, `For`, `Yield` (avoids E0275)
- Inline `Lambda`, `Call` (lexical functions)

### LowLevel (lowered stage)

Unstructured control flow + lifted functions:

- `Arith<ArithType>`, `Cmp<ArithType>`, `Bitwise<ArithType>`, `Constant<ArithValue, ArithType>`
- `Return<ArithType>` — terminator
- `Branch`, `ConditionalBranch` — unstructured CF terminators
- `Bind<ArithType>`, `Call<ArithType>`, `FunctionBody` — lifted functions

### Stage Enum

```rust
#[derive(StageMeta, RenderStage)]
enum Stage {
    #[stage(name = "source")]
    Source(StageInfo<HighLevel>),
    #[stage(name = "lowered")]
    Lowered(StageInfo<LowLevel>),
}
```

### Type System

Reuses `ArithType` / `ArithValue` from `kirin-arith`. Supports i32, i64, f32, f64, bool, index.

## CLI Design

```
toy-lang parse <FILE>
toy-lang run <FILE> --stage <STAGE> --function <FUNC> [ARGS...]
```

- `parse`: pretty-prints the IR to stdout
- `run`: interprets the named function and prints the return value
- Dependencies: `clap` (derive), `anyhow`, `kirin`

## Interpreter Wiring

- `StackInterpreter<ArithValue, _>` for both stages
- Stage dispatch by name: resolve stage ID, match to HighLevel or LowLevel, call `interp.in_stage::<L>().call(func, args)`
- CLI args parsed into `ArithValue` using the function's signature types

## Example Programs

- `add.kirin` — two-argument addition
- `factorial.kirin` — loop-based factorial with structured `for`
- `branching.kirin` — absolute value using `if/else` + comparison

A two-stage example (source → lowered) is deferred to a follow-up.

## Integration Tests

`tests/e2e.rs` using `assert_cmd`:

- `test_add`: `run add.kirin --stage source --function main 3 5` → `8`
- `test_factorial`: `run factorial.kirin --stage source --function factorial 5` → `120`
- `test_abs_positive`: `run branching.kirin --stage source --function abs 42` → `42`
- `test_abs_negative`: `run branching.kirin --stage source --function abs -- -7` → `7`
- `test_parse_roundtrip`: `parse add.kirin` → valid non-empty output
