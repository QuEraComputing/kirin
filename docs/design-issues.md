# Design Issues Tracking

**Date:** 2026-03-07
**Source:** Automated test coverage audit + multi-agent code review + verification pass
**Tests added:** 241 new tests (238 → 479 total)

## Summary

42 findings from 3 reviewer agents were verified against source code. 10 were false positives, 7 were intentional by design, leaving **25 confirmed issues** below.

---

## kirin-ir

### IR-1: BlockBuilder panics on missing terminator (BUG)
**File:** `crates/kirin-ir/src/builder/block.rs`
**Severity:** Medium
`finish()` panics if no terminator was set. Should return `Result` instead.

### IR-2: Arena tombstones accumulate permanently (FOOTGUN)
**File:** `crates/kirin-ir/src/arena/`
**Severity:** Low
Deleted arena entries leave tombstones that are never reclaimed. Not a bug for current workloads but could matter for long-lived pipelines with heavy mutation.

### IR-3: Pipeline panic inconsistency (INCONSISTENCY)
**File:** `crates/kirin-ir/src/pipeline.rs`
**Severity:** Low
Some pipeline accessors panic on missing stages while others return `Option`. Should be consistent — prefer `Option` with panic wrappers.

### IR-4: RegionBuilder O(n) block insertion (FOOTGUN)
**File:** `crates/kirin-ir/src/builder/`
**Severity:** Low
Uses `LinkedList` iteration for block lookups. Fine for small regions but scales poorly.

---

## kirin-lexer

### LEX-2: lex() collapses all errors into one (FOOTGUN)
**File:** `crates/kirin-lexer/src/lib.rs`
**Severity:** Low
`lex()` returns only the first lexer error. Multiple errors in a single input are lost. Acceptable for now but limits error reporting quality.

---

## kirin-chumsky

### C-1: SSA/result field handling duplication (INCONSISTENCY)
**File:** `crates/kirin-chumsky/src/`
**Severity:** Low
SSA value and result fields have similar but not identical handling paths. Minor code smell.

### C-2: String parser/printer asymmetry (INCONSISTENCY)
**File:** `crates/kirin-chumsky/src/builtins/`
**Severity:** Low
String parsing and printing don't perfectly roundtrip for all escape sequences. Intentional convenience trade-off but worth documenting.

### C-3: Float parsers accept integer literals (INCONSISTENCY)
**File:** `crates/kirin-chumsky/src/builtins/`
**Severity:** Low
`f32`/`f64` parsers accept `Token::Int` in addition to `Token::Float`. Intentional (convenience) but undocumented.

### C-5: EmitIR assumes infallible emission (FOOTGUN)
**File:** `crates/kirin-chumsky/src/`
**Severity:** Medium
`EmitIR::emit()` returns `Statement` directly — no `Result`. Implementations that encounter errors must panic. A `Result` return type would be safer.

### C-7: function_symbol panics on invariant violation (BUG)
**File:** `crates/kirin-chumsky/src/`
**Severity:** Low
`function_symbol()` uses `expect()`. The invariant is currently maintained by callers but not enforced by types.

---

## kirin-prettyless

### P-1: strip_trailing_whitespace on all output (FOOTGUN)
**File:** `crates/kirin-prettyless/src/`
**Severity:** Low
All pretty-printed output has trailing whitespace stripped. Could mask formatting bugs.

### P-2: Builder uses expect() on String write (FOOTGUN)
**File:** `crates/kirin-prettyless/src/`
**Severity:** Low
`write!` to `String` can't fail, so `expect()` is technically safe, but inconsistent with the rest of the error handling style.

### P-3: render_function panics on missing function (BUG)
**File:** `crates/kirin-prettyless/src/`
**Severity:** Low
`render_function` panics if the function symbol is not found. Should return `Result` or handle gracefully.

### P-5: GlobalSymbol Display fallback (INCONSISTENCY)
**File:** `crates/kirin-prettyless/src/`
**Severity:** Low
Falls back to raw symbol ID display when name lookup fails. Not a bug but produces confusing output.

---

## kirin-interval

### I-1: saturating_add asymmetry near boundaries (FOOTGUN)
**File:** `crates/kirin-interval/src/`
**Severity:** Low
Protected by `is_empty()` checks in practice, but the raw operation has asymmetric behavior near infinity boundaries.

### I-2: saturating_sub same-infinity subtraction (FOOTGUN)
**File:** `crates/kirin-interval/src/`
**Severity:** Low
Same situation as I-1 — protected by guards but the primitive operation is surprising in isolation.

### I-4: Div/Rem returns top for bottom inputs (INCONSISTENCY)
**File:** `crates/kirin-interval/src/`
**Severity:** Low
Division and remainder operations return `top` (universal interval) when given `bottom` (empty) inputs. Mathematically, `bottom` would be more precise. Sound but imprecise.

---

## kirin-interpreter

### INTERP-1: pending_results desync risk (FOOTGUN)
**File:** `crates/kirin-interpreter/src/`
**Severity:** Medium
`pending_results` vector must stay in sync with SSA result indices. Currently maintained by invariant but not enforced structurally. A mismatch would produce silent wrong results.

### INTERP-3: active_stage_info panics on missing stage (BUG)
**File:** `crates/kirin-interpreter/src/`
**Severity:** Medium
`active_stage_info::<L>()` panics if the active stage doesn't have info for dialect `L`. Should return `Result<_, InterpreterError>`.

### INTERP-5: MissingEntry error reused for different failures (INCONSISTENCY)
**File:** `crates/kirin-interpreter/src/`
**Severity:** Low
`InterpreterError::MissingEntry` is used for both missing entry blocks and missing function entries. Should have distinct variants for better diagnostics.

---

## kirin-derive-chumsky

### DERIVE-CH-1: PrettyPrint codegen panics on missing format (BUG)
**File:** `crates/kirin-derive-chumsky/src/codegen/pretty_print/statement.rs`
**Severity:** Medium
`format_for_statement().expect()` at line 185 panics if a variant lacks a `#[chumsky(format = ...)]` attribute. Should produce a compile error via `syn::Error` instead.

---

## kirin-derive-interpreter

### DERIVE-INT-1: #[callable] behavior change is undocumented (FOOTGUN)
**File:** `crates/kirin-derive-interpreter/`
**Severity:** Medium
Adding or removing `#[callable]` on a variant silently changes the generated `CallSemantics` impl behavior. No compile-time warning when the attribute is missing on a variant that should have it.

### DERIVE-INT-2: First #[callable] variant determines Result type (FOOTGUN)
**File:** `crates/kirin-derive-interpreter/`
**Severity:** Low
When multiple variants have `#[callable]`, the first one's return type determines the `CallSemantics` associated type. Safe in practice (where clause catches mismatches) but surprising.

---

## kirin-derive-toolkit

### DERIVE-TK-1: Unknown stage keys silently ignored (FOOTGUN)
**File:** `crates/kirin-derive-toolkit/src/stage.rs`
**Severity:** Medium
Unrecognized keys in `#[stage(...)]` attributes are silently ignored. Typos like `#[stage(nme = "x")]` produce no error. Should use darling's strict mode or explicit unknown-field rejection.

---

## Priority Recommendations

**Fix soon (Medium severity, high impact):**
1. DERIVE-CH-1 — Panic in derive macro should be compile error
2. DERIVE-TK-1 — Silent attribute typos cause subtle bugs
3. INTERP-3 — Interpreter panic on missing stage info
4. C-5 — EmitIR should return Result
5. INTERP-1 — pending_results structural enforcement

**Fix when touched (Low severity):**
- IR-1, IR-3, C-7, P-3 — Replace panics with Results
- INTERP-5 — Split MissingEntry into distinct variants
- DERIVE-INT-1 — Document #[callable] behavior or add lint

**Track but defer:**
- IR-2, IR-4 — Performance issues, not correctness
- I-1, I-2, I-4 — Sound approximations, imprecise but safe
- C-1, C-2, C-3, P-1, P-2, P-5 — Code quality / documentation
