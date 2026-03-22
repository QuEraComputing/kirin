# U7: Testing, Examples, and Utilities -- Code Review Report

**Date:** 2026-03-22
**Reviewer:** Code Quality + Ergonomics/DX
**Scope:** kirin-test-types, kirin-test-languages, kirin-test-utils, toy-lang, toy-qc, kirin-lexer, kirin-interval

---

## Summary

The testing and examples subsystem is well-structured overall. The three-crate test decomposition (types / languages / utils) successfully breaks the cycle that would otherwise prevent `kirin-chumsky` from using test types. The toy-lang example is a strong showcase: it demonstrates multi-stage pipelines, parse + interpret, structured control flow, and recursive functions. The kirin-interval crate has thorough lattice and arithmetic tests with soundness verification. The main issues are: heavy boilerplate in IR fixture construction, a duplicated test helper, incomplete public API in kirin-interval, and a missing `QubitType` lattice implementation.

---

## Findings

### [P1] [high] `interval_div` and `interval_rem` missing from kirin-interval public API

**File:** `crates/kirin-interval/src/lib.rs:5`
**Perspective:** Code Quality

The `lib.rs` re-exports `interval_add`, `interval_mul`, `interval_neg`, `interval_sub` but omits `interval_div` and `interval_rem`. These are publicly exported from the `interval` submodule (`crates/kirin-interval/src/interval/mod.rs:23-24`) and are tested extensively, but downstream crates using `kirin_interval::interval_div` would get a compile error -- they would need `kirin_interval::interval::interval_div` instead, which leaks internal module structure.

**Suggested action:** Add `interval_div` and `interval_rem` to the re-export line in `lib.rs:5`.

---

### [P1] [high] `QubitType` lacks `Lattice`, `HasBottom`, `HasTop`, `TypeLattice`, and `Display` impls

**File:** `example/toy-qc/src/types.rs:3-13`
**Perspective:** Ergonomics/DX

`QubitType` only derives `Clone, Copy, Debug, Hash, PartialEq, Eq, HasParser, PrettyPrint` and implements `Placeholder`. It has no `Lattice`, `HasBottom`, `HasTop`, `TypeLattice`, or `Display` implementation. The `Dialect` trait requires `Type: CompileTimeValue`, and `CompileTimeValue` requires `Clone + PartialEq + Eq + Hash + Debug + Display` -- the missing `Display` impl means this should not compile unless `Display` is auto-derived or implemented elsewhere. Since it does compile (the kirin prelude likely includes a blanket or the derive macro generates it), the real concern is that the example is incomplete as a teaching artifact: a user following this pattern for their own type lattice would not see how to implement the lattice trait bounds that are required for type checking and abstract interpretation.

**Suggested action:** Add explicit `Display`, `Lattice`, `HasBottom`, `HasTop`, and `TypeLattice` impls to `QubitType` in the example, matching the pattern used by `UnitType` in kirin-test-types. This makes toy-qc a complete reference.

---

### [P2] [high] Duplicated `representative_intervals()` helper function

**File:** `crates/kirin-interval/src/interval/tests.rs:62-76` and `crates/kirin-interval/src/interval/widen_narrow_tests.rs:6-19`
**Perspective:** Code Quality

The `representative_intervals()` function is defined identically in two test files. Both produce the same 11-element `Vec<Interval>`. If the representative set needs updating (e.g., adding graph-body-related interval patterns), it must be changed in two places.

**Suggested action:** Define `representative_intervals()` once in a `#[cfg(test)]` block in the parent `interval/mod.rs` (or a shared test helper submodule) and import it from both test files.

---

### [P2] [high] Heavy boilerplate in `ir_fixtures.rs` -- manual arena manipulation repeated across all fixture builders

**File:** `crates/kirin-test-utils/src/ir_fixtures.rs:111-185` (and lines 194-265, 276-376, 386-475)
**Perspective:** Ergonomics/DX

Every branching fixture (`build_select_program`, `build_branch_fork_program`, `build_loop_program`, `build_infinite_loop`) manually repeats the same low-level pattern: create blocks with arguments, build terminators, manually set `block_arena_mut().get_mut(block).unwrap().terminator = Some(...)`, link statements with `statement_arena_mut()`, set parents explicitly. This pattern appears 25 times in this file. By contrast, the linear fixtures (`build_constants`, `build_add_one`, `build_linear_program`) use the higher-level builder API (`b.block().stmt(c1).stmt(c2).terminator(ret).new()`). The discrepancy suggests the block builder may not support multi-block regions ergonomically, pushing test authors toward raw arena manipulation.

The cost is significant: each new branching test fixture requires ~60 lines of nearly-identical boilerplate, and the pattern is fragile (forgetting to set a parent or terminator causes subtle bugs).

**Suggested action:** Either (a) extend the block builder to support wiring terminators that reference other blocks (so the high-level API works for branching too), or (b) create a helper like `fn wire_terminator(b, block, terminator_stmt)` that encapsulates the three-step set-parent/set-terminator pattern. Even option (b) would cut the fixtures roughly in half.

---

### [P2] [medium] `Token::to_tokens` impl has verbose per-variant boilerplate

**File:** `crates/kirin-lexer/src/lib.rs:148-249`
**Perspective:** Code Quality

The `quote::ToTokens` implementation for `Token` is a 100-line match with every variant manually calling `tokens.extend(quote::quote! { Token::VariantName })`. The punctuation/delimiter variants (LParen through Semicolon) carry no data and could share a single macro invocation or procedural pattern. The data-carrying variants (SSAValue, Block, etc.) do need individual arms, but the ~20 unit variants are pure repetition.

**Suggested action:** Consider using a helper macro to generate the unit-variant arms, or if the feature is rarely used (it is behind `#[cfg(feature = "quote")]`), accept the verbosity with a comment explaining why manual expansion is preferred over a proc macro dependency.

---

### [P3] [medium] `kirin-test-languages` re-exports `SimpleType` and `Value` from `kirin-test-types` but only uses them internally

**File:** `crates/kirin-test-languages/src/lib.rs:1-2`
**Perspective:** Ergonomics/DX

The re-exports `pub use kirin_test_types::SimpleType` and `pub use kirin_test_types::Value` at the top of `kirin-test-languages/src/lib.rs` create a second path to these types. Test authors might import `kirin_test_languages::SimpleType` or `kirin_test_types::SimpleType` interchangeably, which harms discoverability and can cause confusing "type mismatch" errors if the wrong path is used in a generic context. The `SimpleType` is used internally by `simple_language.rs` and `ungraph_language.rs`, but those import via `crate::SimpleType` which resolves through the re-export.

**Suggested action:** Convert the re-exports to `pub(crate) use` so they remain available for internal use without polluting the public API. Consumers who need `SimpleType` should import from `kirin_test_types` directly.

---

### [P3] [medium] `Interval` does not implement `Display`

**File:** `crates/kirin-interval/src/interval/domain.rs:1-68`
**Perspective:** Ergonomics/DX

`Interval` implements `Debug` (derived) but not `Display`. Test failure messages like `not in Interval { lo: Finite(0), hi: PosInf }` are readable but not ideal. A `Display` implementation like `[0, +inf)` or `bot` for empty intervals would make test output and debugging clearer for users of the abstract interpretation framework.

**Suggested action:** Add a `Display` impl that renders intervals in standard mathematical notation (e.g., `[lo, hi]`, with `bot` for empty and `top` for `[-inf, +inf]`).

---

### [P3] [medium] `toy-qc` only supports parse, not interpret

**File:** `example/toy-qc/src/main.rs:28-40`
**Perspective:** Ergonomics/DX

The `toy-qc` CLI only has a `parse` subcommand. The `toy-lang` CLI has both `parse` and `run`. While quantum circuits may not have a natural concrete interpreter, the example misses an opportunity to demonstrate the DiGraph/UnGraph body IR in action. The `Circuit` and `ZX` dialect types also lack `Interpretable` derives. As a teaching example, this leaves a gap -- users looking at toy-qc to learn how to build a graph-body dialect will not see the interpreter side of the story.

**Suggested action:** Either add a simple simulator (even a stub that prints the gate sequence) to demonstrate the interpretable derive on graph-body dialects, or add a doc comment explaining that interpretation is intentionally omitted and pointing to toy-lang for the full pipeline.

---

### [P3] [low] `parse_tokens!` macro re-imports `Parser` trait inside expansion

**File:** `crates/kirin-test-utils/src/lib.rs:33-46`
**Perspective:** Code Quality

The `parse_tokens!` macro has `use $crate::parser::Parser;` inside its expansion body. Since `Parser` is already re-exported as `pub use kirin_chumsky::chumsky::Parser` in `parser.rs:6`, this works. However, the macro also uses `$crate::parser::token_stream` without a `use` -- it calls it fully qualified. The inconsistency (importing `Parser` but fully qualifying `token_stream`) makes the macro slightly harder to follow.

**Suggested action:** Either fully qualify both (`<$crate::parser::token_stream(...)>` and `<$parser as $crate::parser::Parser>::parse(...)`) or import both with `use`.

---

### [P3] [low] `CompositeLanguage` test language is tightly coupled to `ir_fixtures.rs`

**File:** `crates/kirin-test-utils/src/ir_fixtures.rs:10` and `crates/kirin-test-utils/src/lib.rs:22`
**Perspective:** Ergonomics/DX

The `ir_fixtures` module and `dump_function` are hardcoded to `CompositeLanguage`. This means any test that needs IR fixture builders for a different language (e.g., the new `UngraphLanguage`, or a custom dialect with SCF) cannot reuse this infrastructure. Making the fixture builders generic over `L: Dialect` would be nontrivial given the hardcoded `Arith`, `Constant`, `ControlFlow`, and `FunctionBody` references, but the coupling should at least be documented.

**Suggested action:** Add a module-level doc comment to `ir_fixtures.rs` stating that these fixtures are specific to `CompositeLanguage` and explaining when/how to create similar fixtures for other languages.

---

### [P4] [medium] Lexer test for `lex_error_on_invalid_input` only checks one character

**File:** `crates/kirin-lexer/src/lib.rs:543-549`
**Perspective:** Code Quality

The test `test_lex_error_on_invalid_input` only tests `~`. The `test_multiple_error_tokens` test covers `~ ! \``, but there is no test for characters that might interact with existing token patterns (e.g., `|`, `&`, `+`, `-` when not followed by a digit). While the lexer's regex coverage appears solid, testing a few more edge cases at the boundary of valid/invalid would strengthen confidence.

**Suggested action:** Low priority. Consider adding tests for `|`, `&`, `\`, and `+` (which are not tokens in the grammar) to verify they produce errors rather than being silently consumed.

---

### [P4] [low] `#[allow(deprecated)]` in e2e tests without explanation

**File:** `example/toy-lang/tests/e2e.rs:4` and `example/toy-qc/tests/e2e.rs:4`
**Perspective:** Code Quality

Both e2e test files suppress a deprecation warning on `Command::cargo_bin` without a comment explaining why. The `assert_cmd` crate's `Command::cargo_bin` is likely deprecated in favor of a newer API. While functional, the suppression hides a real deprecation.

**Suggested action:** Either migrate to the non-deprecated API (`assert_cmd::Command::cargo_bin` may have been replaced by `CommandCargoExt::cargo_bin`), or add a brief comment explaining the suppression.

---

### [P4] [low] `lattice.rs` assertions use O(n^3) loops without size guards

**File:** `crates/kirin-test-utils/src/lattice.rs:220-243`
**Perspective:** Code Quality

The `check_join_laws` and `check_meet_laws` functions iterate over all triples of elements to verify associativity. With the current usage (8-11 elements), this produces 512-1331 checks, which is fine. But the API accepts `&[L]` without documenting that large inputs will cause cubic blowup. A user testing a type lattice with 50 representative elements would see 125,000 associativity checks per function call.

**Suggested action:** Add a doc comment noting the O(n^3) cost and recommending that callers keep the element count small (under ~20 elements).

---

## Positive Observations

1. **Three-crate test decomposition is well-designed.** The split into kirin-test-types (pure types), kirin-test-languages (dialect enums), and kirin-test-utils (helpers) cleanly solves the circular dependency problem. Feature gating per language in kirin-test-languages is careful and correct.

2. **Lattice law testing infrastructure is excellent.** The `lattice.rs` module collects all violations before panicking, giving a complete failure report rather than stop-on-first-error. The docstrings with examples make it immediately usable.

3. **kirin-interval has property-based soundness tests.** The `test_interval_div_soundness` and `test_interval_rem_soundness` tests exhaustively verify that concrete values fall within computed intervals across all corner cases. The `Bound` arithmetic tests are thorough.

4. **toy-lang is a high-quality example.** It demonstrates multi-stage pipelines (source/lowered), structured control flow (if/else), recursion (factorial), lexical lambdas, and both parse and interpret paths. The e2e tests cover happy paths and error cases (missing function, missing stage).

5. **toy-qc demonstrates DiGraph and UnGraph body IR.** The circuit stage uses directed graphs and the ZX stage uses undirected graphs, showing both graph IR types in a single example with realistic domain modeling.

6. **kirin-lexer has extensive edge-case tests.** Unicode identifiers, integer overflow, float boundary values, punctuation disambiguation, comment nesting, bare prefix characters, and form-feed handling are all tested.
