# Derive (Small) -- Implementer (Code Quality) Review

**Crates:** kirin-derive-ir (885), kirin-derive-interpreter (832), kirin-derive-prettyless (129)
**Total:** ~1846 lines

## Clippy Audit

No `#[allow(...)]` instances in any of these three crates.

## Findings

### DS1. `LocalFieldIterConfig` / `FieldIterConfig` duplication (P3, high confidence)

`kirin-derive-ir/src/generate.rs:15-42` defines `LocalFieldIterConfig` with identical fields to `FieldIterConfig` in `kirin-derive-toolkit`. The `to_field_iter_config()` function (line 218-227) is a field-by-field copy. Same pattern for `LocalPropertyConfig` / `BoolPropertyConfig` (lines 44-63, 229-235). These local wrappers exist because the toolkit types may not be `Copy`, but the conversion is pure boilerplate. Consider making the toolkit configs `Copy` (they are all `&'static str` + small enums) and using them directly, eliminating ~60 lines.

### DS2. `FIELD_ITER_CONFIGS` array could drive the proc-macro registration (P3, low confidence)

`kirin-derive-ir/src/lib.rs:56-73` uses `derive_field_iter_macro!` 14 times with the same pattern. The `FIELD_ITER_CONFIGS` array in `generate.rs:178-193` lists the same 14 configs. These two lists must be kept in sync manually. This is acceptable given the macro approach, but worth noting.

### DS3. `interp_crate` cloned 3 times in interpretable.rs (P3, medium confidence)

`kirin-derive-interpreter/src/interpretable.rs:23,37,69,76` clones `interp_crate` multiple times within closures. This is because closures capture by move and the path is needed in multiple closures. Not a bug, but `Rc<syn::Path>` or restructuring to pass by reference would be slightly cleaner. Minor.

### DS4. `parse_pretty_crate_path` duplicates attribute parsing logic (P3, medium confidence)

`kirin-derive-prettyless/src/generate.rs:43-64` manually parses `#[pretty(crate = ...)]` using `parse_nested_meta`. The toolkit already has `stage::parse_ir_crate_path` for `#[stage(crate = ...)]`. A generic "parse crate path from attribute" helper in the toolkit would serve both. ~20 lines of duplication across prettyless and potentially other derive crates.

## Summary

- 0 `#[allow]` instances
- Code is clean and well-structured; the template system in derive-ir is effective
- Main opportunities are minor: eliminating local config type wrappers and consolidating attribute parsing
- The `derive_field_iter_macro!` / `derive_property_macro!` approach in lib.rs is a good use of declarative macros to reduce proc-macro boilerplate
- Test coverage via insta snapshots is thorough in both derive-interpreter and derive-prettyless
