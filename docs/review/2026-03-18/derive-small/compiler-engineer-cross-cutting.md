# Derive (Small) — Compiler Engineer Cross-Cutting Review

**Crates:** kirin-derive-ir, kirin-derive-interpreter, kirin-derive-prettyless (~1846 lines)

---

## Findings

### DS-CC-1. Good error handling pattern across all three crates
**Severity:** Positive | **Confidence:** High
**Files:** `crates/kirin-derive-ir/src/lib.rs:16-19`, `crates/kirin-derive-interpreter/src/lib.rs:13-16`, `crates/kirin-derive-prettyless/src/lib.rs:11-14`

All three crates consistently use `darling::Error::write_errors()` or `syn::Error::into_compile_error()` for error propagation. This ensures compile errors point at the user's source span rather than the derive internals.

### DS-CC-2. Inconsistent error conversion: `write_errors()` vs `into_compile_error()`
**Severity:** P3 | **Confidence:** High
**Files:** `crates/kirin-derive-ir/src/lib.rs:17` vs `crates/kirin-derive-ir/src/lib.rs:86`

Within `kirin-derive-ir` itself, `Dialect` derive uses `darling::Error::write_errors()` while `StageMeta` and `ParseDispatch` use `syn::Error::into_compile_error()`. This is because the former flows through darling while the latter through the toolkit's `stage` module which returns `syn::Error`. Functionally equivalent but worth noting for consistency.

### DS-CC-3. Clean dependency graph, no unnecessary deps
**Severity:** Positive | **Confidence:** High
**Files:** All three `Cargo.toml` files

All three crates have minimal, identical dependency sets: `kirin-derive-toolkit`, `proc-macro2`, `quote`, `syn`. No extraneous dependencies. Dev-dependencies are `insta` + `kirin-test-utils` only.

### DS-CC-4. `kirin-derive-interpreter` validation produces clear error messages
**Severity:** Positive | **Confidence:** High
**Files:** `crates/kirin-derive-interpreter/src/interpretable.rs:52-67`, `crates/kirin-derive-interpreter/src/eval_call/generate.rs:26-30`

Both `Interpretable` and `CallSemantics` derives validate preconditions (all variants must have `#[wraps]` for Interpretable; at least one `#[callable]` for CallSemantics) and produce error messages naming the offending variants. Tests cover both positive and negative cases.

### DS-CC-5. `kirin-derive-prettyless` uses manual attribute parsing instead of darling
**Severity:** P3 | **Confidence:** Medium
**Files:** `crates/kirin-derive-prettyless/src/generate.rs:43-64`

`parse_pretty_crate_path` manually parses `#[pretty(crate = ...)]` with `parse_nested_meta`. The other derive crates use darling for attribute parsing. This is fine for a single attribute but diverges from the pattern. If more attributes are added to `RenderDispatch`, consider switching to darling.

### DS-CC-6. No `#[diagnostic::on_unimplemented]` on generated trait bounds
**Severity:** P2 | **Confidence:** Medium
**Files:** `crates/kirin-derive-interpreter/src/interpretable.rs:40-41`, `crates/kirin-derive-interpreter/src/eval_call/generate.rs:57-59`

Generated where clauses include `__InterpI: Interpreter<'__ir>` and `__CallSemI::Error: From<InterpreterError>`. When these bounds are unsatisfied, the compiler error references the generated code with mangled names like `__InterpI`. Adding `#[diagnostic::on_unimplemented]` to `Interpreter` and `CallSemantics` traits (in kirin-interpreter, not here) would improve error messages for derive users. This compounds with Phase 1 finding P2-E.

---

**Summary:** The small derive crates are well-structured with consistent patterns and good error diagnostics. The main cross-cutting concern is that generated trait bounds with `__`-prefixed names produce opaque compiler errors when bounds are unsatisfied -- addressable by adding `#[diagnostic::on_unimplemented]` to the target traits in kirin-interpreter.
