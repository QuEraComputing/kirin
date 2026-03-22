# Pre-flight Summary — Full Workspace Refactor

**Refactor scope:** large (14 crates directly modified, 24 affected)
**Pattern:** in-place + additive (SCF result values)
**Source:** Triage review 2026-03-22 (1 P0, 18 P1, 51 P2, 41 P3 accepted)

## Dependency Order (build layers)

```
Layer 0 (no kirin deps): kirin-lexer
Layer 1: kirin-derive-toolkit
Layer 2: kirin-derive-ir, kirin-derive-chumsky, kirin-derive-prettyless, kirin-derive-interpreter
Layer 3: kirin-ir (depends on derive-ir)
Layer 4: kirin-chumsky, kirin-prettyless, kirin-interpreter
Layer 5: kirin-arith, kirin-bitwise, kirin-cf, kirin-cmp, kirin-constant, kirin-function, kirin-scf
Layer 6: kirin-interval, kirin-test-types, kirin-test-languages
Layer 7: kirin-test-utils, kirin (umbrella)
Layer 8: toy-lang, toy-qc, tests/
```

## Changes by Crate

| Crate | Changes | Pub API Impact | Files |
|-------|---------|----------------|-------|
| **kirin-derive-ir** | P0-1: has_signature destructuring | None (codegen fix) | has_signature.rs |
| **kirin-derive-toolkit** | P1-10,11: from_impl fix; P2: FieldData Clone, DeriveContext ToTokens, to_snake_case, BuilderPattern docs | None (codegen fix) | helpers.rs, misc.rs, data.rs, context/mod.rs |
| **kirin-ir** | P1-1,2: detach fix; P1-3: gc() visibility; P1-4: DenseHint resize; P2: detach dedup, BFS dedup, #[must_use], builder rename, SparseHint bounds, GraphInfo docs, semantics unit_cmp | `gc()` → `pub(crate)` (breaking) | detach.rs, arena/gc.rs, arena/hint/dense.rs, arena/hint/sparse.rs, builder/*.rs, signature/semantics.rs |
| **kirin-chumsky** | P1-5,6: RAII scope guards; P1-7,9: panic→Result; P2: port/capture dedup, crate-level allow, Header fields, #[must_use], String escapes, token copy, Signature void, identifier alloc | New ScopeGuard types; fn_symbol returns Result | ast/graphs.rs, ast/blocks.rs, parse_text.rs, parsers/graphs.rs, builtins/primitive.rs, builtins/signature.rs, lib.rs |
| **kirin-prettyless** | P1-16: bat default-features; P1-17: bat Result; P1-18: print_ports dedup; P2: ^name dedup, %name dedup, #[must_use], float NaN, Config style | `print_str` returns Result; bat dep change | Cargo.toml, bat.rs, ir_render.rs, pipeline.rs, impls.rs |
| **kirin-interpreter** | P1-12: try_in_stage(); P1-15: #[must_use]; P2: crate-level allow, is_subseteq assert, frame clone, Vec allocs, FrameStack docs, propagate_control docs | New `try_in_stage()` method | stage_access.rs, control.rs, lib.rs, result.rs, fixpoint.rs, block_eval.rs, frame_stack.rs |
| **kirin-scf** | P1-19,20: If/For result values; P1-21: checked loop_step; P2: induction_var docs, test coverage | Breaking: If/For struct changes, ForLoopValue return type | lib.rs, interpret_impl.rs, tests.rs |
| **kirin-arith** | P2: TryFrom replaces From | Breaking: `From<ArithValue> for i64` → `TryFrom` | types/arith_value.rs |
| **kirin-bitwise** | P2: checked shifts | Breaking: shift ops use try_binary_op | interpret_impl.rs |
| **kirin-function** | P2: Lambda Signature, roundtrip tests | Breaking: Lambda gets `sig` field | lambda.rs, tests.rs |
| **kirin-derive-interpreter** | P2: dedup is_call_forwarding, __Phantom auto-gen | None | eval_call/generate.rs, ssa_cfg_region/generate.rs |
| **kirin-interval** | P1-22: re-exports | None (additive) | lib.rs |
| **kirin-test-***, **tests/** | P2: UnitTy dedup, test cleanup | None (test-only) | Multiple test files |
| **toy-qc** | P3: QubitType docs | None | types.rs |

## Breaking Pub API Changes

1. `Arena::gc()` → `pub(crate)` — consumers must use new compaction API
2. `scf::If<T>` gains `result: ResultValue` field — all constructors/patterns break
3. `scf::For<T>` gains `init_args`/`result` fields — all constructors/patterns break
4. `ForLoopValue::loop_step` → `Option<Self>` — implementors must update
5. `From<ArithValue> for i64` → `TryFrom` — callers must handle error
6. `Lambda<T>` gains `sig: Signature<T>` field — constructors break
7. `bat::print_str` → returns `Result` — callers must handle
8. Shift ops use checked shifts — may return errors where they didn't before

## Visibility Bridges

No one-liner visibility bridges found that would be affected.

## Wave Structure (proposed)

| Wave | Focus | Crates | Dependencies |
|------|-------|--------|-------------|
| 0 | Quick wins (non-breaking) | All | None |
| 1 | Foundation fixes | kirin-derive-toolkit, kirin-derive-ir | None |
| 2 | Core IR fixes | kirin-ir | Wave 1 (derive changes) |
| 3 | Parser/Printer fixes | kirin-chumsky, kirin-prettyless | Wave 2 |
| 4 | Interpreter fixes | kirin-interpreter | Wave 2 |
| 5 | Dialect changes (breaking) | kirin-scf, kirin-arith, kirin-bitwise, kirin-function | Waves 2-4 |
| 6 | Test cleanup | tests/, kirin-test-*, toy-* | Waves 1-5 |
