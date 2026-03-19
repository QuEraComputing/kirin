# RFC 0002: Statement Format Redesign — Generic Result Names and `$keyword` Syntax

- **Status**: Draft
- **Authors**: roger, claude
- **Created**: 2026-03-19
- **Last Updated**: 2026-03-19

## Summary

Three changes to the `#[chumsky(format = "...")]` DSL:

1. **Result names are generic**: `%a, %b =` parsed by the statement parser, not the format string. `{field:name}` for `ResultValue` fields is removed.
2. **Result types stay dialect-controlled**: `{field:type}` for `ResultValue` fields remains — dialects control where types appear.
3. **Keyword syntax changes from `{.name}` to `$name`**: The operation symbol uses `$` prefix instead of `{.}` interpolation. `$name` accepts a valid identifier only — namespacing is added by wrapping dialects, not the format string.

Together these simplify the format DSL and enable multi-result operations naturally.

## Motivation

### The problem

Operations like quantum CNOT produce multiple results:

```
%ctrl_out, %tgt_out = circuit.cnot %ctrl, %tgt -> Qubit, Qubit;
```

Today, the format DSL must express the entire statement including result names:

```rust
#[chumsky(format = "{result:name} = {.h} {qubit} -> {result:type}")]
```

This has two problems:
1. **Multi-result is impossible** — no way to express `{a:name}, {b:name} = ...` with correct LHS/RHS semantics
2. **Result names are boilerplate** — every format string repeats `{result:name} =` even though result names are always `%name` (generic syntax)

However, result **types** must stay dialect-controlled because their position varies:

```
// Type after -> (common)
%x = constant 42 -> i64;

// Type embedded in operation syntax
%y = cast %x to f64;

// Type inferred (no type in text)
z_spider(0.0) %a, %b;
```

### How MLIR solves this

MLIR separates result names from result types ([LangRef](https://mlir.llvm.org/docs/LangRef/)):

- **Result names**: Generic. `%a, %b =` parsed at the operation level, never dialect-controlled.
- **Result types**: Dialect-controlled. ODS `assemblyFormat` uses `type(results)` or `functional-type(...)` directives to place type syntax wherever the dialect needs it.

### Kirin should follow the same split

- **Result names** (`{field:name}` for ResultValue): Remove from format DSL. Parsed generically.
- **Result types** (`{field:type}` for ResultValue): Keep in format DSL. Dialect controls placement.
- **Operand names/types**: No change — stay in format DSL as today.

## Design

### The split: names are generic, types are dialect-controlled

| Aspect | Who handles it | In format DSL? |
|--------|---------------|----------------|
| Result **names** (`%a, %b =`) | Statement parser (generic) | **No** — removed |
| Result **types** (`-> T` or `to T` etc.) | Dialect format string | **Yes** — `{field:type}` stays |
| Operand names/types | Dialect format string | Yes — no change |

### Keyword syntax: `{.name}` → `$name`

The operation keyword changes from `{.name}` (looks like field access) to `$name` (reads as "dialect symbol"):

| Old | New | Meaning |
|-----|-----|---------|
| `{.h}` | `$h` | Simple keyword |
| `{.z_spider}` | `$z_spider` | Underscore keyword |

`$name` accepts a valid Rust identifier only. Namespace prefixes (e.g., `circuit.h`) are added automatically by wrapping dialect enums — individual operations never specify their namespace path in the format string.

### Unified format DSL with three auto-detected modes

`#[chumsky(format = "...")]` serves three purposes, distinguished automatically:

| Mode | Detection | Syntax | Example |
|------|-----------|--------|---------|
| **Statement** | Starts with `$` | `$keyword [operands...] [-> {result:type}...]` | `"$h {qubit} -> {result:type}"` |
| **Type keyword** | Bare literal, no `$`, no `{field}` | literal token(s) | `"Qubit"`, `"i32"` |
| **Body wrapper** | Only `{field}` refs, no `$` | `{field}...` | `"{body}"` |

Detection combines the presence of `#[kirin(type = T)]` with format syntax:

**Primary split**: `#[kirin(type = T)]` present → dialect IR type. Absent → standalone parseable type.

| `#[kirin(type)]`? | Format syntax | Mode | Generated traits |
|-------------------|--------------|------|-----------------|
| **Yes** + `$keyword` | `"$h {qubit} -> {result:type}"` | Statement | HasDialectParser, EmitIR |
| **Yes** + no `$`, body field | `"{body}"` | Body wrapper | HasDialectParser, EmitIR |
| **No** + bare literal | `"Qubit"`, `"i32"` | Type keyword | Display, HasParser, DirectlyParsable, PrettyPrintViaDisplay |
| **No** + has `{field}` | `"Tensor<{elem}>"` | Parameterized type | Display, HasParser, DirectlyParsable, PrettyPrint |

Validation rules:
- If struct has `SSAValue`/`ResultValue` fields, `$keyword` is **required** — compile error without it
- If struct has body fields (Region/DiGraph/UnGraph/Block), `$keyword` is **forbidden**
- Standalone types (no `#[kirin(type)]`) cannot have SSA/Result fields

### Field interpolation and composability

`{field}` in the format string delegates to the field type's `HasParser` implementation. This works uniformly across all modes:

```rust
// Compile-time value in a statement (delegates to Phase::HasParser)
#[chumsky(format = "$rz({angle}) {qubit} -> {result:type}")]
pub struct Rz {
    pub angle: Phase,        // any type implementing HasParser
    pub qubit: SSAValue,
    pub result: ResultValue,
}

// Recursive type parameter (delegates to MyType::HasParser)
#[derive(HasParser, PrettyPrint)]
pub enum MyType {
    #[chumsky(format = "i32")]
    I32,
    #[chumsky(format = "Tensor<{elem}>")]
    Tensor { elem: Box<MyType> },
}

// Body field (delegates to DiGraph parser)
#[kirin(type = QubitType)]
#[chumsky(format = "{body}")]
pub struct CircuitFunction {
    pub body: DiGraph,
}
```

The derive doesn't need to know the field's concrete type — it generates a call to `HasParser::parser()` for whatever type the field has. User-defined compile-time values compose into statement and type formats by implementing `HasParser` + `PrettyPrint`.

### Format DSL changes (complete picture)

Three changes: result `:name` removed, keyword uses `$`, three modes auto-detected.

#### Statement mode (`$keyword`)

```rust
// Single result (current → new):
// Old: #[chumsky(format = "{result:name} = {.h} {qubit} -> {result:type}")]
// New:
#[chumsky(format = "$h {qubit} -> {result:type}")]
// Parses: %q_out = h %q -> Qubit

// Multi-result (impossible today, natural after):
#[chumsky(format = "$cnot {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}")]
pub struct CNOT {
    pub ctrl: SSAValue,
    pub tgt: SSAValue,
    pub ctrl_out: ResultValue,
    pub tgt_out: ResultValue,
}
// Parses: %ctrl_out, %tgt_out = cnot %ctrl, %tgt -> Qubit, Qubit

// Type embedded in operation syntax:
#[chumsky(format = "$cast {input} to {result:type}")]
// Parses: %y = cast %x to f64

// No result types (auto-placeholder):
#[chumsky(format = "$z_spider({angle}) {legs}")]
// Parses: z_spider(0.0) %a, %b

// Zero results (void operation):
#[chumsky(format = "$z_spider({angle}) {legs}")]
// Parses: z_spider(0.0) %a, %b   (no %result = prefix)
```

#### Type keyword mode (bare literal)

```rust
// Type enum variants — bare literal tokens, no $, no {field}
#[derive(HasParser, PrettyPrint)]
pub enum QubitType {
    #[chumsky(format = "Qubit")]
    Qubit,
}

#[derive(HasParser, PrettyPrint)]
pub enum ArithType {
    #[chumsky(format = "i32")]
    I32,
    #[chumsky(format = "i64")]
    I64,
    #[chumsky(format = "f64")]
    F64,
}
// Generates: Display, HasParser, DirectlyParsable, PrettyPrintViaDisplay
```

#### Body wrapper mode (`{field}` only)

```rust
// Function body types — only {field} interpolations, no $
#[chumsky(format = "{body}")]
pub struct CircuitFunction {
    pub body: DiGraph,
}

#[chumsky(format = "{body}")]
pub struct FunctionBody {
    pub body: Region,
}
// The field's type (DiGraph, Region, etc.) determines the parser
```

### Statement parser (generic layer)

The statement parser wraps the dialect parser:

```
1. Try parse: result_name (',' result_name)* '='  → Vec<&str>
2. Delegate to dialect parser (format string)       → dialect AST + parsed types
3. Assign parsed names to ResultValue fields by declaration order
4. Assign parsed types (from format's {field:type}) to ResultValue fields
```

Step 1 is new. Steps 2-4 already happen — the change is that step 1 moves result names OUT of the dialect parser into the statement parser.

The dialect parser returns result types (if any `{field:type}` were in the format) as part of its AST output. The statement parser then pairs names (from step 1) with types (from step 2) and assigns them to `ResultValue` fields by index.

### Backward compatibility

The old format with `{result:name}` is still accepted during transition:
- **New format**: no `{field:name}` for `ResultValue` fields → names parsed generically
- **Legacy format**: `{field:name}` present for `ResultValue` → names parsed by dialect (single-result only, as today)

Detection is automatic in the derive: if any `ResultValue` field has a `:name` occurrence in the format string, use legacy mode.

### EmitIR changes

`ResultValue::emit` currently hardcodes `ResolutionInfo::Result(0)`. With multi-result:
- Each `ResultValue` field gets its index from `HasResults` iteration order (= struct field declaration order)
- The emit codegen passes the index: `ResolutionInfo::Result(field_index)`
- The generic result name parser provides names by position

### PrettyPrint changes

The statement-level printer handles result **names**:
- Print `%name1, %name2 = ` prefix (from SSA info, comma-separated)
- Delegate to dialect `PrettyPrint` for the body (which may include `{field:type}` rendering)

Dialect `PrettyPrint` continues to print result types wherever the format places them. The only change is that result **names** are no longer printed by the dialect — the statement printer does it.

## Alternatives

### Alternative A: Both names AND types in format string (original RFC v1)

Keep everything in the format string, teach codegen about LHS/RHS:

```rust
#[chumsky(format = "{ctrl_out:name}, {tgt_out:name} = {.cnot} {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}")]
```

**Rejected because:** Result names are always `%name` — putting them in the format string is pure boilerplate. The `{.keyword}` syntax looks like field access. Both issues addressed by the main design.

### Alternative B: Remove BOTH names and types from format string

Fully generic statement structure — format string is only keyword + operands:

```rust
#[chumsky(format = "$h {qubit}")]
// Statement parser handles: %result = ... -> Type
```

**Rejected because:** Result types sometimes appear embedded in operation syntax (e.g., `cast %x to f64`). Removing `{field:type}` from the format DSL would prevent this. Type placement must remain dialect-controlled.

### Alternative C: `{results}` group directive

```rust
#[chumsky(format = "{results:name} = $cnot {ctrl}, {tgt} -> {results:type}")]
```

**Rejected because:** `{results:name}` is boilerplate that every format must include. And it still couples names to the format DSL.

### Recommendation

The split approach (main design) is best:
- **Names generic** — no boilerplate, multi-result free
- **Types dialect-controlled** — preserves flexibility for embedded type syntax
- Consistent with MLIR's separation
- Minimal format string changes (just remove `:name` references)

## Crate impact matrix

| Crate | Impact | Changes |
|-------|--------|---------|
| `kirin-chumsky` | **Primary** | Statement parser: add generic result name list parsing before dialect dispatch |
| `kirin-chumsky` | **Primary** | EmitIR: ResultValue emit with index, pair generic names with dialect-parsed types |
| `kirin-derive-chumsky` | **Primary** | Validation: detect new-format (no `{field:name}` for results) vs legacy |
| `kirin-derive-chumsky` | **Primary** | Parser codegen: skip result `:name` in new-format; keep `:type` |
| `kirin-derive-chumsky` | **Primary** | Format parser: recognize `$name` as keyword (replace `{.name}` handling) |
| `kirin-prettyless` | **Secondary** | Statement-level result name printing (extract from dialect PrettyPrint) |
| `kirin-derive-chumsky` | **Secondary** | PrettyPrint codegen: remove result name printing; keep type printing |
| All dialect crates | **Migration** (Phase 2) | Remove `{result:name} =` from format strings; keep `{result:type}`; change `{.name}` to `$name` |

## Migration path

### Phase 1: Add generic result name parsing (non-breaking)
- Statement parser learns to parse `%a, %b =` prefix generically
- Derive detects new-format (no `{field:name}` for results) vs legacy
- New format strings use generic name parsing; old ones work unchanged
- `{field:type}` for results continues to work in both modes

### Phase 2: Migrate existing dialects
- Remove `{result:name} =` from all format strings
- Keep `{result:type}` wherever types appear in the format
- Each dialect format becomes shorter:
  - `"{result:name} = {.h} {qubit} -> {result:type}"` → `"$h {qubit} -> {result:type}"`
- Can be done crate-by-crate

### Phase 3: Remove legacy format support
- Remove `{field:name}` handling for `ResultValue` fields from derive codegen
- `{field:name}` on a `ResultValue` becomes a compile error
- `{field:type}` on a `ResultValue` continues to work (it's the designed path)

## Validation

- **Snapshot tests**: Generated parser/AST/emit code for 0-result, 1-result, 2-result, N-result structs
- **Roundtrip test**: Parse `%a, %b = op %x, %y -> T, T` → print → re-parse
- **Legacy compat test**: Old format strings still work during Phase 1
- **Migration test**: Same dialect with old and new format produces identical IR
- **Existing tests**: All current tests must pass unchanged at every phase

## Open Questions

1. **Result-type elision**: When no `{field:type}` appears in the format for any result, should the statement parser skip `-> Type` entirely? Currently auto-placeholder fills in types. This seems correct — if the dialect doesn't specify type syntax, there's nothing to parse.

2. **Type position validation**: Should the derive validate that `{field:type}` for results appears after operands in the format? Or allow arbitrary placement (like `cast %x to {result:type}`)? Allowing arbitrary placement is more flexible and matches the motivation.

3. **Interaction with graph bodies**: Edge statements in UnGraph use `edge %w = op -> T;`. The `edge` keyword + result names are parsed at the graph level. This should compose naturally — graph parser parses `edge`, then delegates to statement parser which parses `%w =`, then to dialect parser.

4. **Result count mismatch**: What if the text has 2 result names but the struct has 3 `ResultValue` fields? This should be a parse error: "expected 3 results, found 2." The statement parser knows the expected count from the struct's `ResultValue` field count.

## Reference Implementation Plan

### Phase 1 (enable multi-result)
1. **Add `result_name_list` parser** in `kirin-chumsky` — parses `%name (',' %name)* '='` prefix
2. **Wire into statement parser** — call `result_name_list` before dialect parser; pass parsed names to emit layer
3. **Derive detection** in `kirin-derive-chumsky/src/validation.rs` — detect new-format (no `{field:name}` for results) vs legacy
4. **Parser codegen** in `chain.rs` — for new-format, skip result `:name` in dialect chain; keep `:type`
5. **EmitIR** — pass result index to `ResultValue::emit`; pair generic names with dialect-parsed types
6. **PrettyPrint** — statement printer handles `%name1, %name2 = ` prefix; dialect printer handles body including `{field:type}`
7. **Snapshot + roundtrip tests** for 0-result, 1-result, 2-result, embedded-type cases
8. **Migrate toy-qc** — unify CnotCtrl/CnotTgt into single CNOT with 2 results

### Phase 2 (migrate existing dialects)
9. **Remove `{result:name} =` from all format strings** across all dialect crates
10. **Change `{.name}` to `$name`** in all format strings
11. **Verify roundtrip** — same IR produced with new format strings

### Phase 3 (remove legacy)
12. **Make `{field:name}` on ResultValue a compile error** in validation
13. **Make `{.name}` a compile error** in format parser — must use `$name`
14. **Clean up codegen** — remove legacy handling
