# Testing Group -- Final Report

**Crates:** kirin-test-types (348 lines), kirin-test-languages (583 lines), kirin-test-utils (1106 lines)
**Reviewers:** PL Theorist (formalism), Implementer (code quality), Physicist (ergonomics), Compiler Engineer (cross-cutting)
**Total lines:** ~2037

---

## Executive Summary

The testing infrastructure is well-designed. The three-crate split (types / languages / utils) correctly breaks the two-crate-versions dependency cycle, and `kirin-test-types` has clean feature layering. The lattice assertion helpers are a standout -- collecting all violations before reporting. The main actionable findings are: unconditional parser/pretty feature activation in `kirin-test-languages` (P2), making `dump_function` generic (P2), and expanding lattice test coverage beyond the trivial `UnitType` (P3).

| Severity | Accepted | Filtered | Total |
|----------|----------|----------|-------|
| P0       | 0        | 0        | 0     |
| P1       | 0        | 0        | 0     |
| P2       | 2        | 0        | 2     |
| P3       | 4        | 4        | 8     |

---

## Architectural Strengths

1. **Clean feature layering in kirin-test-types** -- Core types compile with only `kirin-ir`; parser and pretty are additive features. This correctly breaks the two-crate-versions cycle. (Compiler Engineer, Physicist)
2. **Lattice assertion helpers** -- `assert_finite_lattice_laws` and friends collect all violations before panicking, providing excellent test ergonomics. The internal `check_*/report` pattern is a good model for other assertion helpers. (PL Theorist, Implementer, Physicist)
3. **Clean roundtrip API** -- Statement and pipeline roundtrip tests require only 3-5 concepts. The API surface is minimal and well-documented. (Physicist)
4. **Correct `kirin-test-utils` dependency on `kirin-test-languages`** -- Only pulled in via the `interpreter` feature, avoiding unnecessary coupling. (Compiler Engineer)

---

## P2 Findings

### T-1. `kirin-test-languages` unconditionally enables `parser` and `pretty` on `kirin-test-types`
**Severity:** P2 | **Confidence:** High | **Reporters:** Compiler Engineer (T-CC-1)
**File:** `crates/kirin-test-languages/Cargo.toml:8`

The dependency `kirin-test-types = { workspace = true, features = ["parser", "pretty"] }` is unconditional. Any feature of `kirin-test-languages` -- even `simple-language` or `ungraph-language` which only need `kirin-ir/derive` -- forces compilation of the full parser (`kirin-chumsky`, `kirin-lexer`) and printer (`kirin-prettyless`) stacks for test types.

**Recommendation:** Make the `parser` and `pretty` features on `kirin-test-types` conditional, activated only by the `kirin-test-languages` features that actually need them (e.g., `arith-function-language`, `callable-language`). The `simple-language` and `ungraph-language` features should not trigger parser/pretty compilation.

### T-2. `dump_function` is hardcoded to `CompositeLanguage`
**Severity:** P2 | **Confidence:** High | **Reporters:** Implementer (T1), Physicist (T4), Compiler Engineer (T-CC-6)
**File:** `crates/kirin-test-utils/src/lib.rs:20-28`

`dump_function` takes `Pipeline<StageInfo<CompositeLanguage>>`. Users testing with other languages (e.g., `ArithFunctionLanguage`) must duplicate this helper.

**Recommendation:** Make the function generic over the language type. The `sprint` method on `SpecializedFunction` already works with any `StageInfo<L>` where `L` satisfies the printer bounds.

---

## P3 Findings

### T-3. Lattice test suite only exercises the trivial `UnitType`
**Severity:** P3 | **Confidence:** High | **Reporters:** PL Theorist (T4)
**File:** `crates/kirin-test-utils/src/lattice.rs:354-357`

`UnitType` is a degenerate one-element lattice where `top == bottom`. All laws hold vacuously. Adding `SimpleType` (7+ elements with non-trivial ordering) would provide meaningful validation of the lattice checker itself.

### T-4. Lattice checker does not verify distributivity
**Severity:** P3 | **Confidence:** Medium | **Reporters:** PL Theorist (T2)
**File:** `crates/kirin-test-utils/src/lattice.rs:138-145`

The checker covers the five standard lattice laws but not the distributive law. Not required for Cousot-style abstract interpretation, but an `assert_distributive_laws` helper would be useful if any analysis relies on distributivity.

### T-5. `parse_tokens!` macro re-export path could use a doc comment
**Severity:** P3 | **Confidence:** Low | **Reporters:** Implementer (T5), Compiler Engineer (T-CC-5)
**File:** `crates/kirin-test-utils/src/lib.rs:34`

The `$crate::parser::Parser` re-export exists solely to support the macro. A brief doc comment explaining this would help maintainability. Acceptable for test-only code.

### T-6. Type lattice boilerplate confirms Phase 1 P1-6
**Severity:** P3 | **Confidence:** High | **Reporters:** Physicist (T2)
**Files:** `crates/kirin-test-types/src/simple_type.rs:15-65`, `crates/kirin-test-types/src/unit_type.rs:6-45`

Even a 1-variant type lattice (`UnitType`) needs ~45 lines of boilerplate across 8 trait impls. This confirms Phase 1 finding P1-6 (type lattice derive support). No separate action needed here -- tracked by P1-6.

---

## Filtered Findings

### Intentional design / known constraints

| Finding | Source | Reason |
|---------|--------|--------|
| Feature fragmentation in test-languages (7 features for 7 languages) | Implementer T2, Compiler Engineer T-CC-2 | The per-language feature flags are intentional for compile-time isolation. An `all-languages` meta-feature could be added but is low priority -- current scale (8 languages) is manageable. |
| `cfg_attr` visual noise in test languages | Physicist T1 | Inherent cost of the feature-gating strategy. The three-crate split is intentional per design context. |
| `SimpleType` Default returns `bottom()` | PL Theorist T3 | Already tracked by Phase 1 P2-D (remove `Default` from `TypeLattice`). No separate action. |
| `Value` lacks `Eq` with manual `Hash` | PL Theorist T5 | Standard `f64` hazard, acceptable for test types. `NaN` inconsistency is well-understood. |

### Low-value / no action needed

| Finding | Source | Reason |
|---------|--------|--------|
| `roundtrip.rs` creates `Pipeline::new()` twice | Implementer T3 | Intentional for roundtrip testing (parse -> print -> parse -> compare). |
| `parse_has_parser` discards span info | Physicist T5 | Tests care about success/failure, not error recovery. Correct for test helpers. |

---

## Suggested Actions (Priority Order)

### Quick Wins (< 30 min)
1. Make `dump_function` generic over language type (T-2)
2. Add `SimpleType` to lattice law test suite (T-3)
3. Add doc comment to `parser::Parser` re-export explaining macro usage (T-5)

### Moderate Effort (1-2 hours)
4. Conditionally activate `parser`/`pretty` features on `kirin-test-types` based on which `kirin-test-languages` features are enabled (T-1)

### Tracked Elsewhere
5. Type lattice derive support (T-6) -- tracked by Phase 1 P1-6
6. Remove `Default` from `TypeLattice` -- tracked by Phase 1 P2-D
