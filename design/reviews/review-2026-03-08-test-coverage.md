# Test Coverage Review — 2026-03-08

## Tests Written

| Crate | File | New Tests | Focus |
|-------|------|-----------|-------|
| kirin-ir | pipeline.rs | 7 | Stage/function/symbol edge cases |
| kirin-ir | builder/context.rs | 17 | link_statements/blocks, empty block/region, iterators, detach, SSA |
| kirin-ir | node/ssa.rs | 7 | Display, roundtrip conversions, From impls |
| kirin-interpreter | frame_stack.rs | 7 | Max depth, empty stack, unbound values |
| kirin-interpreter | frame.rs | 4 | Read/write/into_parts/cursor |
| kirin-interpreter | scheduler.rs | 4 | Empty pop, requeue, len, FIFO order |
| kirin-interpreter | widening.rs | 3 | AllJoins, Never, Delayed threshold |
| kirin-interval | tests.rs | 16 | Overflow saturation, operator traits, meet/join |
| kirin-arith | types/mod.rs | 5 | Type mappings, display, parsing all variants |
| kirin-arith | checked_ops.rs | 8 | Div/rem by zero, overflow, float edge cases |
| kirin-lexer | lib.rs | 34 | Integer/float boundaries, hex, strings, SSA/block/symbol bare sigils, comments, punctuation disambiguation |
| kirin-chumsky | tests.rs | 46 | EmitError display/traits, Spanned API, EmitContext, function type parser, block headers, SSA names, parse_ast edge cases, error handling |
| kirin-prettyless | tests/edge_cases.rs | 30 | Config edge cases, empty pipeline, block args, region rendering, float/string edge cases, RenderError, write_to |
| kirin-prettyless | document/tests.rs | 5 | strip_trailing_whitespace edge cases |
| kirin-derive-toolkit | misc.rs, stage.rs, fields/*.rs, tokens/pattern.rs, hygiene.rs, codegen/utils.rs | 68 | Case conversion, type inspection, stage extraction, field collection/category/info/index, patterns, hygiene, codegen utils |
| kirin-derive-ir | generate.rs | 14 | Union/struct/enum inputs, property validation, StageMeta edge cases |
| kirin-derive-interpreter | interpretable.rs, eval_call/generate.rs | 9 | All non-wraps error, struct wraps, callable variants |
| kirin-cf | lib.rs (tests module) | 18 | Terminator flags, pure/speculatable, successor, arguments/results/blocks/regions |
| kirin-cmp | tests.rs, interpret_impl.rs | 14 | Property traits, argument counts, comparison semantics |
| kirin-bitwise | tests.rs | 15 | Property traits, argument counts, speculatable edge cases |
| kirin-constant | tests.rs | 13 | Constant flag, display, type mappings |
| kirin-function | tests.rs, call.rs, ret.rs | 28 | Call/Return properties, argument structure, terminator flags |
| kirin-scf | tests.rs, interpret_impl.rs | 16 | If/For/While/Yield properties, block containment |
| **Total** | | **~332** | (944 total, up from 612) |

## Findings

### [P2] [certain] `Bound::Finite(i64::MIN).negate()` panics in debug mode — **FIXED**

**File:** `crates/kirin-interval/src/interval/bound.rs:85`

**Resolution:** Used `checked_neg()` and mapped overflow to `Bound::PosInf`. Updated test from `#[should_panic]` to asserting `PosInf` result.

### [P2] [certain] `kirin-scf` does not compile with `--features interpret` — **FIXED**

**File:** `crates/kirin-scf/src/interpret_impl.rs:185`

**Resolution:** Added `L: Interpretable<'ir, I, L> + 'ir` bound to `For` and `StructuredControlFlow` impls. Removed unused `StageAccess` import.

### [P3] [likely] `BoolProperty::for_variant` skips validation — **FIXED**

**File:** `crates/kirin-derive-toolkit/src/template/method_pattern/bool_property.rs`

**Resolution:** Added `self.reader.validate(ctx.input)?` call in `for_variant`. Enum variants now enforce the same `constant-requires-pure` rule as structs.

### [P3] [likely] `String`'s `PrettyPrint` impl doesn't escape inner quotes or newlines — **FIXED**

**File:** `crates/kirin-prettyless/src/impls.rs:295`

**Resolution:** Changed `format!("\"{}\"", self)` to `format!("{:?}", self)` for proper Rust-style escaping.

### [P3] [likely] Lexer skip pattern excludes `\r` (carriage return) — **FIXED**

**File:** `crates/kirin-lexer/src/lib.rs:4`

**Resolution:** Added `\r` to the skip pattern: `[ \t\n\r\f]+`. Windows line endings now work correctly.

## Dropped Findings

- **Interval `Div`/`Rem` always return `top()`** — Intentional conservative approximation.
- **`NegInf + PosInf = NegInf` asymmetric choice** — Already documented with `DESIGN NOTE` comments in existing tests.
