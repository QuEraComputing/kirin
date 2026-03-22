# U2: Parser Runtime (kirin-chumsky) -- Review Report
**Lines:** 6,399 | **Files:** 33
**Perspectives:** Formalism, Code Quality, Ergonomics, Soundness, Dialect Author, Compiler Engineer
**Date:** 2026-03-22

---
## High Priority (P0-P1)

### 1. [P1] [confirmed] Graph emit error paths leak relaxed-dominance mode -- ast/graphs.rs:192-198, 298-309
**Perspective:** Soundness Adversary

In `DiGraph::emit_with` and `UnGraph::emit_with`, `set_relaxed_dominance(true)` is called before emitting statements. If any `emit_statement` call returns `Err`, the `?` operator propagates the error without calling `set_relaxed_dominance(false)` or `pop_scope()`. If the caller catches this error and continues using the same `EmitContext`, all subsequent SSA lookups operate in relaxed-dominance mode, silently creating forward-reference placeholders instead of reporting `UndefinedSSA` errors.

This is a P1 (silent corruption) rather than P0 because the typical pattern is to discard the `EmitContext` on error. However, the API does not enforce this, and a dialect author reusing the context after a partial error would get silently wrong results.

**Suggested action:** Use a scope guard pattern (e.g., a `let _guard = ctx.relaxed_dominance_scope()` RAII type) that restores both the relaxed-dominance flag and the scope stack on drop, ensuring cleanup on all exit paths including `?`.

### 2. [P1] [confirmed] Region emit error path leaks scope -- ast/blocks.rs:249-281
**Perspective:** Soundness Adversary

`Region::emit_with` calls `push_scope()` at line 249 but if `register_block` (line 261) or `emit_block` (line 270) fails with `?`, `pop_scope()` at line 281 is never called. If the `EmitContext` is reused after the error, all subsequent lookups see a phantom inner scope. This is the same class of bug as finding #1 and should be fixed together.

**Suggested action:** Extract a `ScopeGuard` that calls `pop_scope()` on drop, or use a closure-based approach that ensures cleanup.

### 3. [P1] [likely] `parse_text.rs` panics on `link()` failure -- function_text/parse_text.rs:377, 432
**Perspective:** Soundness Adversary

Two calls to `self.link(function, stage_id, staged_function)` use `.expect("link should succeed ...")`. If the link invariant is violated (e.g., the function was already linked to a different staged function in the same stage), this panics inside a parsing API that returns `Result`. All other error paths in the pipeline parsing logic return proper `FunctionParseError`.

**Suggested action:** Replace `.expect(...)` with `.map_err(|e| FunctionParseError::new(FunctionParseErrorKind::EmitFailed, ...))`.

### 4. [P1] [likely] `collect_function_lookup` and `collect_staged_lookup` unwrap arena entries -- function_text/parse_text.rs:778, 819
**Perspective:** Soundness Adversary

Both functions iterate the function arena and call `info.clone().unwrap()`, which panics if the arena entry is a tombstone/None. While current arena implementations may not produce None entries during iteration, this is an unguarded assumption. A future arena refactor or a deleted function could trigger a panic inside the parsing API.

**Suggested action:** Use `filter_map` or handle the None case explicitly instead of `unwrap()`.

### 5. [P1] [confirmed] `fn_symbol` panics on unnamed functions -- function_text/parse_text.rs:808-813
**Perspective:** Soundness Adversary

`fn_symbol` calls `.expect("stage declarations should always use named functions")`. While current code paths ensure the function was created with a name, this is a debug assertion disguised as production code. If any code path creates an unnamed function and passes it here, the parsing API panics.

**Suggested action:** Return a `Result<GlobalSymbol, FunctionParseError>` instead.

---
## Medium Priority (P2)

### 6. [P2] [confirmed] `port_list` and `capture_list` are identical implementations -- parsers/graphs.rs:16-27, 37-48
**Perspective:** Code Quality

These two public functions have identical parser bodies (comma-separated `block_argument` list with trailing comma) and differ only in their `.labelled()` string. The `block_argument_list_bare` function in `parsers/blocks.rs:82-93` is also identical in structure.

**Suggested action:** Unify into a single `bare_argument_list<T>(label: &str)` function and re-export `port_list` and `capture_list` as aliases. Saves ~20 lines and eliminates maintenance divergence.

### 7. [P2] [confirmed] Crate-level `#![allow(clippy::type_complexity, clippy::too_many_arguments)]` -- lib.rs:2
**Perspective:** Code Quality

This crate-wide suppression silences two useful lints globally. While parser combinator types are genuinely complex, the `too_many_arguments` suppression hides real friction (e.g., `apply_stage_declaration` takes 7 parameters, `apply_specialize_declaration` takes 9). Targeted suppression on specific functions would preserve lint value elsewhere.

**Suggested action:** Remove the crate-level `#![allow(...)]` and add `#[allow(clippy::type_complexity)]` only on functions with deeply nested parser return types. For `too_many_arguments`, consider grouping parameters into context structs (which `FirstPassCtx`/`SecondPassCtx` already demonstrate for the dispatch path).

### 8. [P2] [confirmed] `#[allow(dead_code)]` on `Header.stage` and `Header.function` -- function_text/syntax.rs:14-17
**Perspective:** Code Quality

These fields are parsed but never read after the refactor to `parse_declaration_head`. They were part of the original single-pass design. The `Declaration::Specialize` variant also has a `stage` field that is only destructured as `_stage_sym` in `second_pass_concrete` (line 277).

Root cause: `parse_one_declaration` still parses the full header (including `stage` and `function` symbols), but the pipeline loop extracts these from `parse_declaration_head` instead.

**Suggested action:** Either remove the dead fields from `Header` (simplifying the chumsky parser), or start using them in `first_pass_concrete` to avoid the redundant `parse_declaration_head` call. The latter saves the double-parse of the first 4 tokens.

### 9. [P2] [confirmed] No `#[must_use]` on any public type or function -- entire crate
**Perspective:** Code Quality

Key types like `ParseError`, `EmitError`, `ChumskyError`, `FunctionParseError` and functions like `parse_ast`, `parse_and_emit` have no `#[must_use]` annotation. This means callers can silently discard parse errors.

**Suggested action:** Add `#[must_use]` to error types (`ParseError`, `EmitError`, `ChumskyError`, `FunctionParseError`), result-returning public functions (`parse_ast`), and builder-pattern methods that produce values.

### 10. [P2] [confirmed] `String` parser does not handle escape sequences -- builtins/primitive.rs:33-39
**Perspective:** Soundness Adversary

The `String` parser strips surrounding quotes from `StringLit` tokens but does not process escape sequences (`\n`, `\t`, `\\`, `\"`). A string literal `"hello\nworld"` is parsed as the literal bytes `hello\nworld` (with a backslash and `n`), not as a string with a newline. This silently produces incorrect values for any dialect using `String` fields with escaped content.

The lexer regex (kirin-lexer:42) matches escape sequences in the regex but `lex.slice()` returns the raw source text including escape syntax.

**Suggested action:** Add escape-sequence processing in the `String` parser, or provide a separate `unescape()` utility. Document the current behavior if raw strings are intentional.

### 11. [P2] [confirmed] `parse_one_declaration` copies the entire token slice -- function_text/syntax.rs:161
**Perspective:** Compiler Engineer

`Stream::from_iter(tokens.to_vec())` clones the full remaining token slice on every declaration parse in both passes. For a pipeline with N declarations of average length M tokens, this is O(N * total_tokens) total copies in pass 1, plus additional copies in pass 2 for specializations.

**Suggested action:** Pass a subslice reference to chumsky instead of cloning, or restructure the two-pass approach to tokenize once and use index ranges.

### 12. [P2] [confirmed] Signature parser requires `-> T` (no void-return support) -- builtins/signature.rs:23
**Perspective:** Dialect Author

The `Signature<T>` parser requires `-> T` after the parameter list. A dialect author defining a void-returning function type like `fn @foo()` cannot express this. The `function_type` parser (parsers/function_type.rs:30-48) handles the optional arrow case, but `Signature`'s parser does not.

**Suggested action:** Make the `-> T` portion optional in `Signature::parser()`, defaulting to `T::placeholder()` or requiring the type lattice to have a unit/void representation.

### 13. [P2] [confirmed] `identifier()` allocates a `format!` string on every call -- parsers/identifiers.rs:21
**Perspective:** Compiler Engineer

`identifier(name)` calls `.labelled(format!("identifier '{}'", name))`, which allocates a `String` on every parser construction. Since parser combinators are typically constructed once and reused, this is a minor issue, but it is unnecessary given that `name` is `&'t str`.

**Suggested action:** Use `concat!` or a `Cow<str>` label if chumsky supports it, or accept the allocation as negligible for one-time construction.

---
## Low Priority (P3)

### 14. [P3] [confirmed] `Spanned<T>` ignores span in `PartialEq` -- ast/spanned.rs:15-19
**Perspective:** Formalism

The `PartialEq` impl for `Spanned<T>` only compares `.value`, ignoring `.span`. This is intentional for AST comparison in tests (comparing parsed output regardless of source location), but violates the expectation that `PartialEq` compares all semantically relevant fields. A type named `Spanned` that ignores its span in equality is surprising.

**Suggested action:** Document this explicitly (a doc comment on the `PartialEq` impl) and consider providing a `span_eq` method for cases where span comparison is needed.

### 15. [P3] [confirmed] Forward-reference SSAs created with `ResolutionInfo::Result(0)` -- traits/emit_ir.rs:219
**Perspective:** Formalism

`create_forward_ref` always uses `ResolutionInfo::Result(0)` for forward references, regardless of the actual result index. When the `ResultValue::emit` later resolves the forward ref (values.rs:115-117), it updates the result index. However, if the forward ref is never resolved (the SSA name is used but never defined as a result), it silently carries `Result(0)` rather than failing. This is inherent to relaxed-dominance mode but worth documenting.

**Suggested action:** Add a validation pass after graph emit that checks all forward-reference SSAs were resolved.

### 16. [P3] [confirmed] `EmitContext` scope invariants rely on `assert!` panics -- traits/emit_ir.rs:88-97
**Perspective:** Soundness Adversary

`pop_scope` panics via `assert!` if the root scope would be popped. Since `EmitContext` is a public type and `push_scope`/`pop_scope` are public methods, a dialect author misusing these could panic. This is appropriate (programming error), but returning `Result` would be more consistent with the crate's error philosophy.

**Suggested action:** Informational only. The panic is defensible since mismatched push/pop is a bug, not a runtime condition.

### 17. [P3] [confirmed] Redundant `Clone` bound on `HasParser::Output` -- traits/has_parser.rs:24
**Perspective:** Ergonomics/DX

`HasParser<'t>` requires `Output: Clone + PartialEq`. The `Clone` bound propagates through all parser combinators and AST types. For large AST outputs, this forces cloning in combinator chains even when the value could be moved. Chumsky itself requires `Clone` on parser outputs for backtracking, so this is unavoidable with the current chumsky version, but it contributes to the generic-bound cascade.

**Suggested action:** Informational. The bound reflects chumsky's requirement.

### 18. [P3] [confirmed] `HasDialectEmitIR` has heavy documentation for `#[doc(hidden)]` trait -- traits/has_dialect_emit_ir.rs:1-74
**Perspective:** Ergonomics/DX

This trait is marked `#[doc(hidden)]` and documented as "implementation detail of derive-generated code," yet it has 50+ lines of documentation explaining its design rationale. While this is useful for maintainers, it adds to the conceptual weight when browsing source. Consider moving the rationale to a `// DESIGN NOTE:` comment block rather than doc comments, since `#[doc(hidden)]` items should not appear in public API docs.

**Suggested action:** Move design rationale from `///` to `//` comments to reduce noise in IDE hover docs.

### 19. [P3] [confirmed] `TestDialect` defined inline in tests.rs despite kirin-test-languages existing -- tests.rs:383-388
**Perspective:** Code Quality

A minimal `TestDialect` enum and `TestType` struct are defined inline for `EmitContext` tests. The crate instructions say to put test types in `kirin-test-types` and test dialects in `kirin-test-languages`. However, the MEMORY notes the two-crate-versions problem prevents `kirin-chumsky` from using `kirin-test-types` with parser features. This inline definition is the pragmatic workaround.

**Suggested action:** Informational. The workaround is necessary due to the two-crate-versions cycle.

### 20. [P3] [confirmed] `best_stage_suggestion` computes Levenshtein twice per candidate -- function_text/parse_text.rs:931-937
**Perspective:** Compiler Engineer

`min_by_key` computes `levenshtein(stage_symbol, c)`, then `filter` computes it again. For the typical handful of stage candidates this is negligible, but it is an easy fix.

**Suggested action:** Compute once: `candidates.iter().map(|c| (c, levenshtein(stage_symbol, c))).min_by_key(|(_, d)| *d).filter(|(_, d)| *d <= 3).map(|(c, _)| c.clone())`.

---
## Strengths

1. **Clean trait layering.** The `HasParser` / `HasDialectParser` / `ParseEmit` / `ParseDispatch` trait hierarchy is well-decomposed. Each trait has a clear responsibility and they compose without leaking implementation details.

2. **Two-pass region emit.** The stub-block creation in `Region::emit_with` (pass 1: register block names for forward references, pass 2: emit block bodies) is a correct and clean solution to the forward block reference problem in SSA IRs.

3. **Error taxonomy.** The crate has three distinct error types (`ParseError`, `EmitError`, `ChumskyError`) that correctly distinguish syntax errors from semantic errors. `FunctionParseError` adds structured categories via `FunctionParseErrorKind` with span information. The error chain (`source()`) is properly threaded.

4. **Scope-based SSA/block name management.** `EmitContext`'s scope stack with shadowing semantics correctly models nested scoping (regions, graph bodies). The relaxed-dominance mode for graph bodies is well-motivated.

5. **Comprehensive test coverage.** The test files cover parser combinators, error paths, edge cases (empty input, overflow, wrong prefix), `EmitContext` semantics, and the full pipeline parse round-trip. The `function_text/tests.rs` file exercises multi-stage dispatch, stage suggestions, and error chains.

6. **Graph parsing refactor quality.** The recent `GraphInfo` unification produced clean `DiGraph`/`UnGraph` AST types and parsers that share infrastructure (`collect_port_info`, `graph_header`) without excessive abstraction.

7. **Monomorphic dispatch.** `ParseDispatch` eliminates HRTB from the pipeline parsing path, which is a significant improvement for both compile times and error message quality.

---
## Filtered Findings

The following patterns were considered but not flagged per the Design Context exclusion list:

- Single lifetime `HasParser<'t>` -- intentional collapse of old two-lifetime system
- Witness methods `clone_output`/`eq_output` on `HasDialectParser` -- GAT E0275 workaround
- `ParseEmit<L>` three implementation paths -- intentional flexibility
- `ParseDispatch` monomorphic dispatch design -- intentional HRTB elimination
- `Ctx` default parameter on `ParseStatementText` -- intentional unified trait pattern
- `parse_and_emit` conflating `EmitError` with `ParseError` via `ChumskyError` -- previously noted P2
- `#[wraps]` working with Region/Block types -- old E0275 resolved
