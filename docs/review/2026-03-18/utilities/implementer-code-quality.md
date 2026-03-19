# Utilities -- Implementer (Code Quality) Review

**Crates:** kirin-lexer (991), kirin-interval (1454)
**Total:** ~2445 lines

## Clippy Audit

No `#[allow(...)]` instances in either crate.

## Findings

### U1. `Token::Display` and `Token::ToTokens` both exhaustively match all variants (P2, medium confidence)

`kirin-lexer/src/lib.rs:93-130` (Display) and `kirin-lexer/src/lib.rs:144-243` (ToTokens) both enumerate all 28+ variants. Adding a new token variant requires updating both match arms plus the Logos derive. The `Display` impl cannot be derived, but `ToTokens` could potentially be generated via a macro or derive since every variant follows the pattern `Token::X => quote! { Token::X }` (or `Token::X(v) => quote! { Token::X(#v) }`). This would eliminate ~100 lines and prevent the two matches from drifting.

### U2. `lex()` error message loses the token text (P3, medium confidence)

`kirin-lexer/src/lib.rs:136-137`: On error, `lex()` produces `"Unexpected token at position {span.start}"` but does not include the actual character(s) that failed to lex. Including the source slice (e.g., `&input[span]`) would improve diagnostic quality.

### U3. `Interval` public fields allow invalid construction (P2, medium confidence)

`kirin-interval/src/interval/domain.rs:5-8`: `lo` and `hi` are `pub` fields. The `new()` constructor normalizes `lo > hi` to bottom, but direct field access bypasses this. Code like `Interval { lo: Bound::Finite(10), hi: Bound::Finite(5) }` creates a state that `new()` would have rejected. `is_empty()` handles this correctly, but operations like `interval_add` may not expect it. Consider `pub(crate)` fields with `lo()` / `hi()` accessors.

### U4. `Interval::bottom_interval()` naming (P3, low confidence)

`kirin-interval/src/interval/domain.rs:26`: Named `bottom_interval()` rather than just `bottom()`. This may be intentional to avoid conflict with a `HasBottom::bottom()` trait method, but it creates an inconsistency -- users see `Interval::bottom_interval()` vs `Interval::bottom()` (from the trait). The standalone constructor could be `pub(crate)` with `HasBottom::bottom()` as the public API.

### U5. `StringLit` variant allocates on every lex (P3, low confidence)

`kirin-lexer/src/lib.rs:42`: `StringLit(String)` calls `.to_string()` in the regex callback, allocating a heap String for every string literal. All other data-carrying variants borrow from the input (`&'src str`). For a lexer that processes large inputs, this is a potential performance concern. However, string literals in IR text are likely rare, so this is minor.

### U6. Thorough test coverage (positive note)

`kirin-lexer` has exceptionally thorough tests (~500 lines) covering edge cases like bare sigils, unicode, scientific notation, dot disambiguation, and error recovery. `kirin-interval` similarly has extensive test modules across 5 test files. Both crates demonstrate good testing practices.

## Summary

- 0 `#[allow]` instances in either crate
- `kirin-lexer` is clean and well-tested; main opportunity is reducing `ToTokens` boilerplate
- `kirin-interval` has a public-fields concern that could lead to invariant violations
- Both crates are well-structured with clear module boundaries
