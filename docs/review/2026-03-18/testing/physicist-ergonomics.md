# Testing -- Physicist (Ergonomics/DX) Review

**Crates:** kirin-test-types, kirin-test-languages, kirin-test-utils
**Lines:** ~2037

## Scenario: "I want to test my dialect with roundtrip tests"

The roundtrip helper API is clean. For statement-level: call `roundtrip::assert_statement_roundtrip::<MyDialect>(input, operands)`. For pipeline-level: call `roundtrip::assert_pipeline_roundtrip::<MyLanguage>(input)`. The arith roundtrip test (`tests/roundtrip/arith.rs`) demonstrates both paths clearly.

## Concept Budget: Roundtrip Testing

| Concept | Required? | Where learned |
|---------|-----------|---------------|
| `roundtrip::assert_statement_roundtrip` | Statement tests | kirin-test-utils |
| `roundtrip::assert_pipeline_roundtrip` | Pipeline tests | kirin-test-utils |
| `roundtrip::emit_statement` + `render_statement` | Custom assertions | kirin-test-utils |
| Operand setup `&[("name", Type)]` | Statement tests | roundtrip.rs |
| Pipeline text format (stage/specialize) | Pipeline tests | text format |

**Total: 3-5 concepts** depending on whether custom assertions are needed. This is good.

## Findings

### T1. `cfg_attr` layering in test languages adds visual noise (P3, medium confidence)

`kirin-test-languages/src/arith_function_language.rs:9-13` requires 5 lines of `#[cfg_attr(...)]` to conditionally derive parser/printer traits. Every test language repeats this pattern. This is inherent to the feature-gating strategy (test types without parser deps) and is not easily simplified without losing the flexibility. Acceptable cost.

### T2. Type lattice boilerplate for test types confirms P1-6 (P2, high confidence)

`SimpleType` (`kirin-test-types/src/simple_type.rs`) requires: Lattice (3 methods), HasBottom, HasTop, TypeLattice (marker), Placeholder, Display, Default = 8 trait impls totaling ~80 lines. For `UnitType` (`unit_type.rs`): same 8 impls, ~45 lines. Adding parser support adds HasParser + DirectlyParsable = 2 more impls. This confirms the P1-6 finding: even a 1-variant type lattice needs ~45 lines of boilerplate.

**Files:** `kirin-test-types/src/simple_type.rs:15-65`, `kirin-test-types/src/unit_type.rs:6-45`

### T3. Lattice test helpers are well-designed and composable (strength)

`kirin-test-utils/src/lattice.rs` provides granular helpers (assert_join_laws, assert_meet_laws, assert_absorption, etc.) plus a comprehensive `assert_finite_lattice_laws`. All violations are collected and reported together rather than failing on first. This is exactly what a physicist running lattice tests would want.

### T4. `dump_function` helper is hardcoded to CompositeLanguage (P3, low confidence)

`kirin-test-utils/src/lib.rs:21-28`: The `dump_function` helper is parameterized over a fixed `CompositeLanguage` type. Users testing with different languages need to write their own. Consider making this generic, though the function is small enough that copy-paste is acceptable.

### T5. parse_has_parser helper discards span info (P3, low confidence)

`kirin-test-utils/src/parser.rs:25-36`: The `parse_has_parser` function converts errors to strings, losing span information. For test helpers this is fine -- tests care about success/failure, not error recovery. No action needed.
