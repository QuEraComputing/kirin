# Implementer -- Code Quality Review: kirin-ir

## Clippy Workaround Audit

| Location | Allow Type | Reason | Classification | Action |
|----------|-----------|--------|---------------|--------|
| `src/builder/region.rs:31` | `allow(clippy::wrong_self_convention, clippy::new_ret_no_self)` | Builder pattern: `fn new(self) -> Region` consumes self and returns the built type, not `Self`. | genuinely needed | Keep -- standard builder pattern idiom |
| `src/builder/block.rs:93` | `allow(clippy::wrong_self_convention, clippy::new_ret_no_self)` | Same builder pattern: `fn new(self) -> Block`. | genuinely needed | Keep |
| `src/builder/digraph.rs:84` | `allow(clippy::wrong_self_convention, clippy::new_ret_no_self)` | Same builder pattern: `fn new(self) -> DiGraph`. | genuinely needed | Keep |
| `src/builder/ungraph.rs:77` | `allow(clippy::wrong_self_convention, clippy::new_ret_no_self)` | Same builder pattern: `fn new(self) -> UnGraph`. | genuinely needed | Keep |
| `src/signature/semantics.rs:61` | `allow(clippy::unit_cmp)` | Compares `call.constraints == cand.constraints` where `C` defaults to `()`. The comparison is generic over `C: PartialEq` and only triggers `unit_cmp` when `C = ()`. | fixable with refactoring | Could gate the constraints comparison on a `C: PartialEq` bound with a non-`()` marker, but the allow is pragmatic. **Low priority.** |
| `src/signature/semantics.rs:97` | `allow(clippy::unit_cmp)` | Same issue in `LatticeSemantics::applicable`. | fixable with refactoring | Same as above |
| `src/signature/mod.rs:2` | `allow(clippy::module_inception)` | Module `signature` contains file `signature.rs`. | fixable with refactoring | Rename inner file to `definition.rs` or `types.rs` |
| `tests/common.rs:9` | `allow(dead_code)` | `TestType` is used by tests in sibling files but not all variants are exercised. Standard test helper issue. | genuinely needed | Keep -- test utility code |
| `tests/common.rs:45` | `allow(dead_code)` | `BuilderDialect` enum -- same reasoning. | genuinely needed | Keep |
| `tests/common.rs:208` | `allow(dead_code)` | `make_wire` function -- test helper. | genuinely needed | Keep |
| `tests/common.rs:224` | `allow(dead_code)` | `new_stage` function -- test helper. | genuinely needed | Keep |

## Logic Duplication

### 1. DiGraphBuilder and UnGraphBuilder port/capture allocation (P2, confirmed)

**Files:** `src/builder/digraph.rs:82-133` and `src/builder/ungraph.rs:82-124`

The port and capture allocation loops are nearly identical across both builders:
- Port SSA creation loop (ports, then captures)
- `port_name_to_index` and `capture_name_to_index` HashMap construction
- Replacement map building and application (resolve `Unresolved` SSAs to real port SSAs)

Both share the same fields (`ports`, `captures`, `name`, `parent`) and the same builder methods (`port`, `port_name`, `capture`, `capture_name`, `name`, `parent`).

**Suggestion:** Extract a `GraphPortBuilder` helper struct or a shared method that handles port/capture allocation and replacement resolution. This would reduce ~100 lines of duplication across the two files.

### 2. Name resolution pattern in ir_render.rs (P3, confirmed)

**File:** `kirin-prettyless/src/document/ir_render.rs`

The pattern of resolving a name from the symbol table appears in `print_block` (line 52-60), `print_ports` (line 114-122), and `print_digraph` yield section (line 178-186). Each occurrence:
```
let name = if let Some(name_sym) = info.name() {
    self.stage.symbol_table().resolve(name_sym).cloned()
        .unwrap_or_else(|| format!("{}", Id::from(*x).raw()))
} else {
    format!("{}", Id::from(*x).raw())
};
```
This is a cross-crate observation (reported in prettyless review too).

## Rust Best Practices

### Missing `#[must_use]` annotations (P2, confirmed)

Zero `#[must_use]` annotations in the entire crate. Key candidates:
- `Id::raw()` -- pure accessor
- `Signature::placeholder()` -- constructor
- All `GetInfo` methods (`get_info`, `expect_info`)
- Builder methods returning `Self` (already consumed by chaining, so lower priority)
- `BuilderStageInfo::finalize()` which returns `Result<StageInfo<L>, FinalizeError>`

### `format!` in Display positions (P3, confirmed)

**File:** `src/signature/semantics.rs` and other locations

`format!("{}", Id::from(*arg).raw())` could just use `.to_string()` or write directly to the formatter. Minor allocation concern.

### Builder panics instead of Results (P3, confirmed)

**Files:** `src/builder/block.rs:69-73`, `src/builder/block.rs:83-87`, `src/builder/mod.rs:53-68`

`BlockBuilder::stmt()` and `BlockBuilder::terminator()` use `assert!` to validate terminator status. `resolve_builder_key` uses `panic!` for out-of-bounds and missing names. These are construction-time checks that could return `Result` for better composability, though panics are arguably acceptable for builder APIs where misuse is a programming error.

### `Signature` fields are `pub` (P3, confirmed)

**File:** `src/signature/signature.rs:8-11`

`Signature { pub params, pub ret, pub constraints }` -- all fields are public. This prevents future invariant enforcement. Consider accessor methods.

## Summary

- P2 confirmed -- `src/builder/digraph.rs` + `src/builder/ungraph.rs`: ~100 lines of duplicated port/capture/replacement logic should be extracted
- P2 confirmed -- Missing `#[must_use]` across the crate (zero instances)
- P2 confirmed -- `src/signature/semantics.rs:61,97`: `unit_cmp` allows are fixable by restructuring the constraint comparison
- P3 confirmed -- `src/signature/mod.rs:2`: `module_inception` allow, rename inner file
- P3 confirmed -- Builder APIs use panics where Results could improve composability
