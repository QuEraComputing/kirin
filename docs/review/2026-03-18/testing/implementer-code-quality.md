# Testing -- Implementer (Code Quality) Review

**Crates:** kirin-test-types (348), kirin-test-languages (583), kirin-test-utils (1106)
**Total:** ~2037 lines

## Clippy Audit

No `#[allow(...)]` instances in any testing crate.

## Findings

### T1. `dump_function` is hardcoded to `CompositeLanguage` (P2, high confidence)

`kirin-test-utils/src/lib.rs:21-28`: `dump_function` takes `Pipeline<StageInfo<CompositeLanguage>>` -- it cannot be used with any other test language. This should be generic over `L: Dialect + PrettyPrint` where `L::Type: Display`, matching the pattern used by `render_statement`. Users of other test languages (e.g., `ArithFunctionLanguage`) must duplicate this helper.

### T2. Feature fragmentation in test-languages (P3, low confidence)

`kirin-test-languages/src/lib.rs` gates every language behind its own feature flag (7 features for 7 languages). Each integration test must enable the exact features it needs. This is thorough but creates friction -- a `full` or `all` feature would be convenient for tests that need multiple languages. Not a bug, just a DX observation.

### T3. `roundtrip.rs` creates `Pipeline::new()` twice per test (P3, low confidence)

`kirin-test-utils/src/roundtrip.rs:75,85`: `assert_pipeline_roundtrip` creates two `Pipeline::new()` instances and parses twice. This is intentional (it's a roundtrip test), but the function does not return the rendered string, making it hard to debug failures. Consider returning the rendered output on success, or at least including it in the panic message on assertion failure.

### T4. `lattice.rs` helpers are well-designed (positive note)

The lattice test helpers (`assert_finite_lattice_laws`, etc.) collect all violations before panicking, which is excellent test ergonomics. The internal `check_*` / `report` pattern at `kirin-test-utils/src/lattice.rs:23-32` is a good model for other test assertion helpers in the workspace.

### T5. `parse_tokens!` macro uses `$crate::parser::Parser` (P3, low confidence)

`kirin-test-utils/src/lib.rs:34`: The macro imports `Parser` trait via `$crate::parser::Parser`. This re-export exists only to support the macro. A doc comment explaining this re-export purpose would help maintainability.

## Summary

- 0 `#[allow]` instances
- Test infrastructure is clean and well-factored
- Main actionable item: make `dump_function` generic (T1)
- The three-crate split (types / languages / utils) effectively breaks dependency cycles
- Lattice assertion helpers are a standout for test quality
