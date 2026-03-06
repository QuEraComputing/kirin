# Test Restructure Design

**Date:** 2026-03-06
**Status:** Approved

## Problem

Tests are scattered across many crates with no clear conventions:

1. **Duplicated helpers** — `assert_roundtrip()`, `NumericLanguage`, `CallableLanguage` defined in multiple test files
2. **Hard to navigate** — roundtrip tests in kirin-arith/tests, kirin-bitwise/tests, kirin-function/tests, workspace tests/, kirin-chumsky tests; unclear where new tests should go
3. **Hard to write** — boilerplate for setting up types, stages, pipelines repeated in every test file
4. **Coverage gaps** — no codegen snapshots for `{.keyword}` namespace feature
5. **Two-crate-versions problem** — kirin-test-utils can't share types with kirin-test-languages because of transitive kirin-chumsky dependency

## Design

### Three Test Crates

**`kirin-test-types`** (new) — Pure type definitions, no parser/printer deps:
- `UnitType` (moved from kirin-test-utils)
- `SimpleType` with I64, F64, Bool (moved from kirin-test-languages)
- `SimpleValue` (moved from kirin-test-languages)
- Trait impls: `Display`, `Lattice`, `HasParser`, `PrettyPrint`
- **Key constraint:** Depends on kirin-ir and kirin-chumsky for trait impls but NOT on kirin-test-utils (breaks the cycle)

**`kirin-test-languages`** (refactored) — Dialect and stage enums:
- `SimpleLanguage` (existing)
- `CompositeLanguage` (existing)
- `NumericLanguage` (consolidate from arith/bitwise tests — wraps Arith with Return)
- `CallableLanguage` (consolidate from function tests — wraps Call, Bind, Return)
- `NamespacedLanguage`, `BareLanguage` (consolidate from namespace_roundtrip.rs)
- Test stage enums

**`kirin-test-utils`** (refactored) — Helpers and fixtures:
- `roundtrip` module: `assert_statement_roundtrip()`, `assert_pipeline_roundtrip()`
- `ir_fixtures` module: `build_add_one()`, `build_select_program()`, etc.
- `parser` module: `tokenize()`, `parse_tokens!`, `parse_has_parser()`
- `lattice` module: `assert_finite_lattice_laws()`
- `rustfmt` module

### Workspace `tests/` Structure

All parse/print roundtrip tests move to workspace `tests/`:

```
tests/
  roundtrip/
    mod.rs              # shared imports and setup
    arith.rs            # arith dialect statement + pipeline roundtrips
    bitwise.rs          # bitwise dialect roundtrips
    cf.rs               # control flow (br, cond_br) roundtrips
    cmp.rs              # comparison (eq, ne, lt, le, gt, ge) roundtrips
    constant.rs         # constant roundtrips
    function.rs         # call, bind, ret, lambda roundtrips
    scf.rs              # structured control flow (if, for, yield) roundtrips
    namespace.rs        # namespace prefix roundtrips (existing namespace_roundtrip.rs)
    composite.rs        # multi-dialect pipeline roundtrips
  simple.rs             # existing full-pipeline roundtrip
```

### What Stays in Crates

Only **inline unit tests** (`#[cfg(test)]` modules) for crate-internal logic:

| Crate | What stays |
|-------|-----------|
| `kirin-ir` | Signature matching, stage dispatch |
| `kirin-lexer` | Token lexing |
| `kirin-derive-chumsky` | Format parser tests, codegen snapshot tests |
| `kirin-interval` | Lattice law tests, widen/narrow tests |
| `kirin-interpreter` | Interpreter execution, abstract interpretation, stage dispatch |
| `kirin-prettyless` | Document rendering, sprint, output writing |
| `kirin-chumsky` | Basic parser combinator tests (identifiers, symbols, SSA names) |

### What Moves Out

| Current location | Destination | Content |
|-----------------|-------------|---------|
| `kirin-arith/tests/arith.rs` | `tests/roundtrip/arith.rs` | Statement roundtrips |
| `kirin-arith/tests/function_roundtrip.rs` | `tests/roundtrip/arith.rs` | Pipeline roundtrips |
| `kirin-bitwise/tests/bitwise.rs` | `tests/roundtrip/bitwise.rs` | Statement roundtrips |
| `kirin-bitwise/tests/function_roundtrip.rs` | `tests/roundtrip/bitwise.rs` | Pipeline roundtrips |
| `kirin-function/tests/function_roundtrip.rs` | `tests/roundtrip/function.rs` | Call, bind, ret roundtrips |
| `kirin-function/tests/lambda_print.rs` | `tests/roundtrip/function.rs` | Lambda roundtrips |
| `tests/namespace_roundtrip.rs` | `tests/roundtrip/namespace.rs` | Namespace prefix tests |
| `NumericLanguage` (inline in arith/bitwise) | `kirin-test-languages` | Shared test dialect |
| `CallableLanguage` (inline in function) | `kirin-test-languages` | Shared test dialect |
| `NamespacedLanguage` (inline in namespace_roundtrip) | `kirin-test-languages` | Shared test dialect |
| `UnitType` | `kirin-test-types` | Test type |
| `SimpleType`, `SimpleValue` | `kirin-test-types` | Test types |

### New Codegen Snapshot Tests

Add to `kirin-derive-chumsky` inline tests:

1. Snapshot for keyword parser codegen: `{.add}` → namespace-branching parser chain
2. Snapshot for keyword pretty-print codegen: `{.add}` → namespace-branching text output
3. Snapshot for wrapper namespace extension: `#[wraps]` + `format = "arith"` → namespace push + delegation

### Dependency Graph

```
kirin-test-types
  ├── kirin-ir (types, traits)
  ├── kirin-chumsky (HasParser)
  └── kirin-prettyless (PrettyPrint)

kirin-test-languages
  ├── kirin-test-types (type defs)
  ├── kirin-ir
  ├── kirin-chumsky (derive HasParser)
  ├── kirin-prettyless (derive PrettyPrint)
  └── kirin-arith, kirin-cf, kirin-function, etc. (dialect crates)

kirin-test-utils
  ├── kirin-ir
  ├── kirin-chumsky (parse helpers)
  ├── kirin-prettyless (print helpers)
  └── kirin-test-types (type defs for fixtures)

workspace tests/
  ├── kirin (top-level crate)
  ├── kirin-test-types
  ├── kirin-test-languages
  └── kirin-test-utils
```

### Conventions Going Forward

1. **Roundtrip tests** → workspace `tests/roundtrip/<dialect>.rs`
2. **Unit tests for internal logic** → inline `#[cfg(test)]` in the crate
3. **Codegen snapshot tests** → inline in derive crate
4. **IR rendering snapshots** → kirin-prettyless inline tests
5. **New test type** → `kirin-test-types`
6. **New test dialect** → `kirin-test-languages`
7. **New test helper** → `kirin-test-utils`
