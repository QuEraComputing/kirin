# Testing — Compiler Engineer Cross-Cutting Review

**Crates:** kirin-test-types, kirin-test-languages, kirin-test-utils (~2037 lines)

---

## Findings

### T-CC-1. `kirin-test-languages` unconditionally enables `parser` and `pretty` features on `kirin-test-types`
**Severity:** P2 | **Confidence:** High
**Files:** `crates/kirin-test-languages/Cargo.toml:8`

`kirin-test-types` is declared as `kirin-test-types = { workspace = true, features = ["parser", "pretty"] }` in `kirin-test-languages`. This means ANY feature of `kirin-test-languages` -- even `simple-language` or `ungraph-language` which need only `kirin-ir/derive` -- pulls in the full parser and printer stacks for test types. Since `kirin-test-types`'s `parser` feature depends on `kirin-chumsky` and `kirin-lexer`, and `pretty` depends on `kirin-prettyless`, this inflates compile times for test configurations that do not need parsing.

**Recommendation:** Move the `features = ["parser", "pretty"]` to be conditional on the language features that actually need them, or make this a default that individual features can override.

### T-CC-2. Feature flag explosion in `kirin-test-languages`
**Severity:** P2 | **Confidence:** Medium
**Files:** `crates/kirin-test-languages/Cargo.toml:23-32`

Eight feature flags with overlapping dependency sets. For example, `arith-function-language`, `bitwise-function-language`, `callable-language`, and `namespaced-language` all require `kirin-ir/derive` + `parser` + `pretty` and differ only in which dialect crates they pull in. At 50 dialects this becomes O(N) features. Consider a single `all-languages` feature or a pattern where each language module independently activates its dialect deps.

### T-CC-3. `kirin-test-utils` depends on `kirin-test-languages` only for `interpreter` feature
**Severity:** Positive | **Confidence:** High
**Files:** `crates/kirin-test-utils/Cargo.toml:12-13`

`kirin-test-languages` is correctly optional in `kirin-test-utils`, only pulled in by the `interpreter` feature. The `roundtrip` and `parser` features avoid this dependency.

### T-CC-4. `kirin-test-types` has clean feature layering
**Severity:** Positive | **Confidence:** High
**Files:** `crates/kirin-test-types/Cargo.toml:12-15`

Three features (`default = []`, `parser`, `pretty`) with minimal deps. Core types (`UnitType`, `SimpleType`, `Value`) compile with only `kirin-ir`. This was correctly designed to break the two-crate-versions cycle.

### T-CC-5. Macro hygiene issue: `parse_tokens!` exposes internal paths
**Severity:** P3 | **Confidence:** Medium
**Files:** `crates/kirin-test-utils/src/lib.rs:32-46`

The `parse_tokens!` macro references `$crate::parser::Parser` and `$crate::parser::token_stream` which are re-exports. If the underlying crate changes its API, the macro breaks. This is acceptable for test-only code but fragile for a public macro.

### T-CC-6. `dump_function` helper is tightly coupled to `CompositeLanguage`
**Severity:** P3 | **Confidence:** Medium
**Files:** `crates/kirin-test-utils/src/lib.rs:20-28`

The `dump_function` helper is hardcoded to `Pipeline<StageInfo<CompositeLanguage>>`. A generic version parameterized by the language type would be more reusable as more test languages are added.

---

**Summary:** The primary concern is that `kirin-test-languages` unconditionally forces parser+printer compilation on `kirin-test-types` regardless of which language features are enabled (T-CC-1). The feature flag system will also need rethinking at scale (T-CC-2). The `kirin-test-types` crate itself is well-layered.
