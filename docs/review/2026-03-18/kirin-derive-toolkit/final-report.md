# kirin-derive-toolkit — Final Review Report

## High Priority (P0-P1)

No high-priority issues were identified. The crate is well-structured with clean error handling, correct abstraction boundaries, and sound architectural decisions.

## Medium Priority (P2)

### 1. Missing `#[must_use]` on builder-returning methods
**Source:** Implementer report.
**Details:** `Input::compose()` returns a `TemplateBuilder` that is useless unless `.build()` is called. Zero `#[must_use]` annotations exist in the crate. While this is internal derive infrastructure (not user-facing runtime code), a forgotten `.build()` would silently produce no output.
**Action:** Add `#[must_use]` to `Input::compose()` and `TemplateBuilder::build()`.

### 2. Derive macro discoverability for new dialect authors
**Source:** Physicist report.
**Details:** A full-featured dialect with interpreter support requires up to 10 derives (`Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint, Interpretable, SSACFGRegion`). The relationship between `#[callable]` and `SSACFGRegion` is especially non-obvious. This is a documentation/onboarding issue, not a code issue.
**Action:** Add a "which derives do I need?" cheat sheet to dialect authoring documentation. Consider a convenience derive macro (e.g., `#[derive(KirinDialect)]`) that implies the standard set, though this is lower priority than documentation.

## Low Priority (P3)

### 3. No duplicate detection for `#[stage(name = "...")]` values
**Source:** Compiler engineer report. **Confirmed** by code inspection of `stage_info.rs`.
**Details:** If two variants share the same `#[stage(name = "source")]` string, `from_stage_name` will match the first arm and silently ignore the second. No compile-time diagnostic is emitted. This would cause subtle runtime bugs.
**Action:** Add a duplicate-name check in `generate()` (collect names into a `HashSet`, error on collision). Low priority because this is an unlikely user mistake, but the fix is trivial.

### 4. `__`-prefixed variant filtering is an implicit convention
**Source:** PL theorist report. **Confirmed.**
**Details:** `Input::from_derive_input` silently skips variants starting with `__`, triggering a wildcard `_ => unreachable!()` arm. No documentation or attribute marks this behavior. The `__` prefix convention is strong enough that collisions are unlikely, but the implicit nature could surprise users.
**Action:** Document this convention. Optionally, add an explicit `#[kirin(hidden)]` attribute as an alternative.

### 5. `StatementExtra` unused in `StandardLayout`
**Source:** PL theorist report. **Confirmed.**
**Details:** `Layout::StatementExtra` is `()` in `StandardLayout` and flows through the entire `Statement<L>` type without contributing value in the common case. This is a minor abstraction leak for future extensibility.
**Action:** No action needed. The cost is negligible and the extensibility point may be needed later.

### 6. `syn` extra-traits feature always enabled
**Source:** Compiler engineer report.
**Details:** `syn` is compiled with `features = ["extra-traits", "full"]` workspace-wide, adding `Debug`/`Eq`/`Hash` impls to all AST types. This adds baseline compile cost. However, `darling` likely requires these features.
**Action:** Investigate whether `extra-traits` can be made conditional. Low priority — likely blocked by darling's requirements.

## Strengths

1. **Clean monoidal template composition.** The `Template<L>` trait with `Vec<TokenStream>` output and `TemplateBuilder` composition is simple, correct, and extensible. Factory methods (`bool_property`, `field_iter`, `marker`) cover common patterns well.

2. **Layout type family pattern.** The `Layout` trait with associated types for per-derive attribute extensibility is the idiomatic Rust encoding. Each derive crate gets its own attribute namespace without coupling.

3. **DeriveContext memoization.** Pre-computing `StatementContext` (wrapper types, binding patterns, field classifications) once and sharing across all templates avoids redundant work and ensures consistency.

4. **Excellent error diagnostics.** Consistent use of `darling::Error` and `syn::Error::new_spanned` produces compile errors that point to the offending source location. Stage parsing errors are specific and actionable.

5. **No runtime cost.** The crate is purely compile-time infrastructure with zero impact on runtime dependency graphs.

6. **Both `#[allow(clippy::large_enum_variant)]` annotations are justified.** `Data<L>` and `FieldData<L>` are constructed once during derive processing; boxing would add complexity without meaningful benefit.

7. **Clean error propagation.** No `unwrap()` or `expect()` in non-test code paths. All errors flow through `darling::Result` or `syn::Result`.

## Filtered Findings

| Finding | Source | Reason for filtering |
|---------|--------|---------------------|
| Template method patterns have duplication | Implementer | False positive. Each pattern (`bool_property`, `field_iter`, `delegate`, `custom`) generates genuinely different code. The template system's extension pattern is intentional. |
| `Custom<L>::separate()` closure pattern | PL theorist (implicit) | Intentional architecture per design context. |
| DeriveContext pre-computing StatementContext | PL theorist | Intentional architecture per design context. Correctly identified as a strength, not an issue. |
| `pub` vs `pub(crate)` visibility | Implementer | Not an issue. `pub` is needed for cross-crate access within the derive workspace. |
| `.clone()` on syn types | Implementer | Expected and correct for proc-macro codegen. `syn` types are designed for this. |
| Attribute namespace fragmentation (5+ namespaces) | Physicist | Architecturally sound per design context (each derive owns its namespace; `#[wraps]`/`#[callable]` are intentionally separate for composability). The cognitive load is real but is a documentation issue, not a code issue. Folded into finding #2. |
| O(V * 21) generated match arms | Compiler engineer | Informational only. Linear scaling is acceptable for derive macro output. No dialect in practice approaches 50 variants. |
| Legacy Scan/Emit still exists | (implicit) | Intentional — still used by kirin-derive-chumsky per design context. |

## Suggested Follow-Up Actions

1. **Quick win:** Add `#[must_use]` to `Input::compose()` and `TemplateBuilder::build()`. (Finding #1, ~5 min)
2. **Quick win:** Add duplicate `#[stage(name)]` detection in `stage_info.rs::generate()`. (Finding #3, ~15 min)
3. **Documentation:** Create a dialect authoring quick-reference showing which derives, attributes, and trait bounds are needed for common scenarios. (Finding #2)
4. **Documentation:** Document the `__`-prefixed variant convention. (Finding #4)
