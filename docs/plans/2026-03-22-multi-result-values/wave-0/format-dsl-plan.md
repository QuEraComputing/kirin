# Text Format DSL — `[...]` Optional Section Syntax

**Finding(s):** W2
**Wave:** 0
**Agent role:** Implementer
**Estimated effort:** design-work

---

## Issue

The text format DSL (`#[chumsky(format = "...")]`) needs a new `[...]` optional section syntax. This enables void-if (`if %cond then {..} else {..}` with no result) and zero-or-more results (`[ -> {results:type}]`).

Currently there is no way to mark part of a format string as optional. Operations like `scf.if` always require a result type even when the if is void (side-effect only). The `[...]` syntax wraps optional groups that are parsed as all-or-nothing units.

**Crate(s):** kirin-derive-chumsky, kirin-lexer (for `[[`/`]]` escaping tokens)
**File(s):**
- `crates/kirin-lexer/src/lib.rs` — add `EscapedLBracket` and `EscapedRBracket` token variants for `[[` and `]]`
- `crates/kirin-derive-chumsky/src/format.rs` — format string parser (add `[...]` element)
- `crates/kirin-derive-chumsky/src/validation.rs` — validation rules for optional sections
- `crates/kirin-derive-chumsky/src/codegen/parser/chain.rs` — parser codegen (wrap in `.or_not()`)
- `crates/kirin-derive-chumsky/src/codegen/pretty_print/statement.rs` — printer codegen (wrap in `if field.is_some()`)
- `crates/kirin-derive-chumsky/src/visitor.rs` — FormatVisitor may need optional section awareness

**Confidence:** confirmed

## Guiding Principles

- "Chumsky Parser Conventions": Format strings define the syntax for parsing and printing dialect statements via `#[chumsky(format = "...")]`.
- "No unsafe code": All implementations MUST use safe Rust.
- "Derive Infrastructure Conventions": `mod.rs` should stay lean. Move substantial logic into sibling files.
- Escaping convention: `[[` produces literal `[`, `]]` produces literal `]` (consistent with `{{`/`}}` for braces).

## Scope

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-lexer/src/lib.rs` | modify | Add `#[token("[[")]  EscapedLBracket` and `#[token("]]")] EscapedRBracket` token variants (following the `EscapedLBrace`/`EscapedRBrace` pattern at lines 52-57), plus Display, ToTokens, and test coverage |
| `crates/kirin-derive-chumsky/src/format.rs` | modify | Add `FormatElement::Optional(Vec<FormatElement>)` variant, parse `[...]` syntax, handle `[[`/`]]` escaping via `EscapedLBracket`/`EscapedRBracket` tokens |
| `crates/kirin-derive-chumsky/src/validation.rs` | modify | Add rules: Option fields must be inside `[...]`, required (bare) fields cannot be inside `[...]`, no nesting |
| `crates/kirin-derive-chumsky/src/codegen/parser/chain.rs` | modify | Wrap optional section parser chain in `.or_not()` |
| `crates/kirin-derive-chumsky/src/codegen/pretty_print/statement.rs` | modify | Wrap optional section printer in `if field.is_some()` |
| `crates/kirin-derive-chumsky/src/visitor.rs` | modify | Add `visit_optional_section` or handle optional groups in iteration |

**Files explicitly out of scope:**
- `crates/kirin-derive-toolkit/` — builder template changes are in wave-0/builder-template-plan.md
- `crates/kirin-scf/`, `crates/kirin-function/` — dialect changes are in wave-2 plans

## Verify Before Implementing

- [ ] **Verify: FormatElement enum location**
  Run: `grep -n "pub enum FormatElement" crates/kirin-derive-chumsky/src/format.rs`
  Expected: Single hit around line 97
  If this fails, STOP — format.rs structure may have changed.

- [ ] **Verify: Lexer has LBracket/RBracket tokens** (CONFIRMED)
  The lexer already has `Token::LBracket` (line 59) and `Token::RBracket` (line 61) in `crates/kirin-lexer/src/lib.rs`. It does NOT yet have `EscapedLBracket`/`EscapedRBracket` for `[[`/`]]` — those must be added (following the `EscapedLBrace`/`EscapedRBrace` pattern at lines 52-57).

- [ ] **Verify: FormatVisitor trait shape**
  Run: `grep -n "fn visit" crates/kirin-derive-chumsky/src/visitor.rs | head -10`
  Expected: Shows visitor method signatures to understand extensibility.

- [ ] **Verify: parser codegen chain builder**
  Run: `grep -n "fn.*chain\|build_chain\|parser_chain" crates/kirin-derive-chumsky/src/codegen/parser/chain.rs | head -10`
  Expected: Shows the chain builder that generates parser combinator chains from format elements.

## Regression Test

- [ ] **Write regression test for `[...]` parsing**
  Add a test in `format.rs` tests module that parses a format string containing `[...]`:
  ```rust
  #[test]
  fn test_optional_section() {
      let input = "$if {condition} then {then_body} else {else_body}[ -> {result:type}]";
      let format = Format::parse(input, None).expect("Failed to parse format");
      insta::assert_debug_snapshot!(format);
  }
  ```
  Before implementation: this will fail because `[` is not handled.

- [ ] **Run the test — confirm it fails**
  Run: `cargo nextest run -p kirin-derive-chumsky -E 'test(test_optional_section)'`
  Expected: FAIL — `[` is not a recognized format element.

## Design Decisions

**Decision 1: FormatElement representation**
- **Primary approach:** Add `FormatElement::Optional(Vec<FormatElement<'src>>)` variant. The `[...]` section contains nested format elements. This is a recursive structure but nesting is disallowed by validation.
- **Fallback:** Use `FormatElement::OptionalStart` / `FormatElement::OptionalEnd` markers (flat representation). This avoids recursion but makes codegen harder.
- **How to decide:** The recursive variant is cleaner for codegen (map over inner elements). Use it.

**Decision 2: Lexer token for `[` and `]`** (RESOLVED)
- `Token::LBracket` and `Token::RBracket` already exist in the lexer (lines 58-61).
- `Token::EscapedLBracket` (`[[`) and `Token::EscapedRBracket` (`]]`) do NOT exist yet. Add them following the `EscapedLBrace`/`EscapedRBrace` pattern (lines 52-57). The Logos `#[token("[[")]` attribute handles the longest-match disambiguation against single `[`.

**Decision 3: Vec<ResultValue> inside `[...]` semantics**
- **Primary approach:** When a `Vec` field appears inside `[...]`, absence of the optional section means the Vec receives an empty `Vec::new()`. Presence means the comma-separated list is parsed into the Vec.
- **Fallback:** N/A — this is the design doc specification.
- **How to decide:** Implement directly per design doc.

**Decision 4: Vec fields outside `[...]` and zero-element parsing**
- The current codegen for `Vec<T>` fields uses `.separated_by(Comma).allow_trailing().collect()`, which requires at least one element.
- A `Vec<SSAValue>` field outside `[...]` that must accept zero elements (e.g., `$yield {values}` where `yield` alone means empty Vec) cannot parse zero elements with the current codegen.
- **Primary approach:** When a `Vec` field is the ONLY field reference after the keyword (nothing follows it in the format), allow zero elements by wrapping the separated_by in `.or_not().map(|v| v.unwrap_or_default())`. Alternatively, dialect authors can use `$yield[ {values}]` to explicitly make the values optional.
- **How to decide:** If the zero-element case is needed immediately (Wave 2 Yield needs it), implement the `.or_not()` wrapping. Otherwise, document that zero-element Vec fields should use `[...]`.

**Decision 5: Multiple independent `[...]` sections**
- **Primary approach:** Support multiple `[...]` sections in one format string. Each is independent. Fields in different optional sections are validated separately.
- **Fallback:** Limit to one optional section per format string.
- **How to decide:** The design doc says "Multiple `[...]` sections are independent." Implement multiple.

## Implementation Steps

- [ ] **Step 1: Add escaped bracket tokens to lexer**
  In `crates/kirin-lexer/src/lib.rs`, add `EscapedLBracket` and `EscapedRBracket` variants. `LBracket`/`RBracket` already exist (lines 58-61). Add the new variants after `RBracket` (line 61), following the `EscapedLBrace`/`EscapedRBrace` pattern:
  ```rust
  #[token("[[")]
  EscapedLBracket,
  #[token("]]")]
  EscapedRBracket,
  ```
  Also add Display impls (around line 112), ToTokens impls (around line 199), and test coverage (around line 461).
  Run: `cargo clippy -p kirin-lexer && cargo nextest run -p kirin-lexer`
  Expected: Clean build and all tests pass.

- [ ] **Step 2: Add `FormatElement::Optional` variant**
  In `format.rs`, add:
  ```rust
  /// An optional section `[...]` — parsed as all-or-nothing.
  Optional(Vec<FormatElement<'src>>),
  ```

- [ ] **Step 3: Update the format parser for `[...]`**
  In `Format::parser()`, add an optional section parser that:
  - Matches `Token::LBracket`
  - Recursively parses inner elements (reuse the element parser)
  - Matches `Token::RBracket`
  - Maps to `FormatElement::Optional(inner)`
  Also add escaped bracket handling: `[[` -> literal `[`, `]]` -> literal `]`.

- [ ] **Step 4: Write format parsing tests**
  Add snapshot tests for:
  - `"$if {cond} then {body1} else {body2}[ -> {result:type}]"` — optional result type
  - `"$call {target}({args})[ -> {results:type}]"` — optional results
  - `"$for {iv} in {start}..{end}[[ step {step} ]]"` — escaped brackets (literal)
  - `"$op {x}[ -> {a:type}][ -> {b:type}]"` — multiple optional sections
  Run: `cargo nextest run -p kirin-derive-chumsky -E 'test(optional)'`
  Expected: All pass with correct snapshots.

- [ ] **Step 5: Update validation for optional sections**
  In `validation.rs`, add these rules (matching the design doc section "Validation rules"):
  1. Every field reference inside `[...]` must have collection type `Option<T>` or `Vec<T>` — a bare (non-collection) field inside `[...]` is a compile error.
  2. An `Option<T>` field reference that appears in the format string but NOT inside any `[...]` section is a compile error (ambiguous which tokens are optional). `Vec<T>` fields can appear outside `[...]` (they handle emptiness via separator).
  3. `[...]` cannot be nested — a `[` inside an already-open `[...]` section (that is not the escaped `[[`) is a compile error.
  4. Multiple `[...]` sections in one format string are independently validated.

- [ ] **Step 6: Write validation tests**
  Add tests that verify:
  - Bare (non-collection) field inside `[...]` -> compile error
  - `Option<T>` field referenced in format string but outside `[...]` -> compile error
  - Nested `[` inside already-open `[...]` (not escaped `[[`) -> compile error
  - `Vec<T>` field outside `[...]` -> OK (no error)
  Run: `cargo nextest run -p kirin-derive-chumsky -E 'test(validation)'`
  Expected: All pass.

- [ ] **Step 7: Update parser codegen for optional sections**
  In `codegen/parser/chain.rs` (or relevant codegen file), when encountering `FormatElement::Optional`:
  - Generate a sub-parser chain for the inner elements
  - Wrap in `.or_not()`
  - Map the result: when `Some(...)`, populate the fields; when `None`, set Option fields to `None` and Vec fields to `Vec::new()`

- [ ] **Step 8: Update pretty print codegen for optional sections**
  In `codegen/pretty_print/statement.rs`, when encountering `FormatElement::Optional`:
  - Generate `if` check: if any field in the section is `Some`/non-empty, print the entire section
  - The condition should check the "primary" field (the first field reference in the optional section)

- [ ] **Step 9: Write end-to-end codegen snapshot tests**
  Add a test struct that uses `[...]` syntax and verify the generated parser + printer code.
  Run: `cargo insta test -p kirin-derive-chumsky`
  Expected: New snapshots for optional section codegen.

- [ ] **Step 10: Run full crate tests**
  Run: `cargo nextest run -p kirin-derive-chumsky`
  Expected: All tests pass.

- [ ] **Step 11: Run workspace build**
  Run: `cargo build --workspace`
  Expected: Clean build. No downstream crate uses `[...]` yet.

- [ ] **Step 12: Fix clippy warnings**
  Run: `cargo clippy -p kirin-derive-chumsky`
  Expected: No warnings.

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations to suppress warnings — fix the underlying cause.
- Do NOT leave clippy warnings. Run `cargo clippy -p kirin-derive-chumsky` before completion.
- Do NOT support nested `[...]` — the design doc explicitly disallows it.
- Do NOT modify dialect struct definitions (kirin-scf, kirin-function) — those are in wave-2 plans.
- Do NOT change the builder template — that is in wave-0/builder-template-plan.md.
- No unsafe code (AGENTS.md: all implementations MUST use safe Rust).

## Validation

**Per-step checks:**
- After step 4: `cargo nextest run -p kirin-derive-chumsky -E 'test(optional)'` — Expected: snapshots match
- After step 6: `cargo nextest run -p kirin-derive-chumsky -E 'test(validation)'` — Expected: all pass
- After step 10: `cargo nextest run -p kirin-derive-chumsky` — Expected: all pass
- After step 11: `cargo build --workspace` — Expected: clean build

**Final checks:**
```bash
cargo clippy -p kirin-lexer                    # Expected: no warnings (escaped bracket tokens)
cargo nextest run -p kirin-lexer              # Expected: all tests pass
cargo clippy -p kirin-derive-chumsky          # Expected: no warnings
cargo nextest run -p kirin-derive-chumsky     # Expected: all tests pass
cargo build --workspace                        # Expected: clean build
cargo test --doc -p kirin-derive-chumsky      # Expected: all doctests pass
```

**Snapshot tests:** yes — run `cargo insta test -p kirin-derive-chumsky` and report changes, do NOT auto-accept.

## Success Criteria

1. Format strings with `[...]` syntax parse correctly into `FormatElement::Optional` nodes.
2. `[[` and `]]` produce literal bracket characters (escaping works).
3. Validation catches: required fields inside `[...]`, Option fields outside `[...]`, nested `[...]`.
4. Parser codegen wraps optional sections in `.or_not()` and correctly maps absent sections to `None`/empty Vec.
5. Pretty print codegen conditionally prints optional sections based on field presence.
6. Existing format strings without `[...]` are unaffected (no regression).

**Is this a workaround or a real fix?**
This is the real fix. The `[...]` syntax is a new format DSL feature specified in the design document, enabling void operations and zero-or-more results in the text format.
