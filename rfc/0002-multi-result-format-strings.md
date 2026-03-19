# RFC 0002: Multi-Result Statements with Generic Result Syntax

- **Status**: Draft
- **Authors**: roger, claude
- **Created**: 2026-03-19
- **Last Updated**: 2026-03-19

## Summary

Separate result parsing/printing from the dialect format string. Following MLIR's design, the result list (`%a, %b =`) and result types (`-> T, T`) are handled by the **statement parser** generically, not by individual dialect format strings. The format DSL only describes the operation body (keyword + operands). This enables multi-result operations naturally and simplifies the format DSL.

## Motivation

### The problem

Operations like quantum CNOT produce multiple results:

```
%ctrl_out, %tgt_out = circuit.cnot %ctrl, %tgt -> Qubit, Qubit;
```

Today, the format DSL must express the entire statement including results:

```rust
#[chumsky(format = "{result:name} = {.h} {qubit} -> {result:type}")]
```

This couples the generic statement structure (results, `=`, `->`, types) with the dialect-specific body (keyword, operands). Multi-result operations cannot be expressed because the format DSL has no concept of "LHS results vs RHS operands."

### How MLIR solves this

MLIR separates the operation syntax into two layers ([LangRef](https://mlir.llvm.org/docs/LangRef/)):

```
operation ::= op-result-list? (generic-operation | custom-operation)
op-result-list ::= op-result (',' op-result)* '='
```

The result list and `=` are parsed at the **generic operation level**. Dialect `assemblyFormat` in ODS only describes the RHS — the keyword, operands, and attributes. Result names are always `%name`, handled by the generic parser. Result types are specified via `type(results)` or `functional-type(...)` directives, but the comma-separated result list is never dialect-controlled.

### Kirin should follow the same separation

The format DSL should describe only the **operation body**:

```rust
// Before (current): format string includes results and types
#[chumsky(format = "{result:name} = {.h} {qubit} -> {result:type}")]

// After (proposed): format string is only the operation body
#[chumsky(format = "{.h} {qubit}")]
```

The statement parser handles: `%result =` prefix, `-> Type` suffix, and comma-separated multi-result.

## Design

### Statement structure (generic layer)

Every statement follows this structure, parsed by the statement-level parser:

```
[%result1, %result2, ...] = [dialect.keyword operands...] [-> Type1, Type2, ...]
```

Where:
- **Result list** (0 or more `%name`): parsed generically, comma-separated, count matches `ResultValue` fields
- **`=`**: present if any results exist
- **Operation body**: parsed by the dialect format string
- **`->`**: present if any results have types
- **Type list**: parsed generically, comma-separated, count matches result count

### Format DSL changes

The format string drops all result-related syntax:

```rust
// Single-result operation (current)
#[chumsky(format = "{result:name} = {.h} {qubit} -> {result:type}")]
// Becomes:
#[chumsky(format = "{.h} {qubit}")]

// Multi-result operation (impossible today, natural after)
#[chumsky(format = "{.cnot} {ctrl}, {tgt}")]
pub struct CNOT {
    pub ctrl: SSAValue,
    pub tgt: SSAValue,
    pub ctrl_out: ResultValue,
    pub tgt_out: ResultValue,
}
// Parses: %ctrl_out, %tgt_out = circuit.cnot %ctrl, %tgt -> Qubit, Qubit

// Zero-result operation
#[chumsky(format = "{.z_spider}({angle}) {legs}")]
pub struct ZSpider {
    pub angle: f64,
    pub legs: Vec<SSAValue>,
}
// Parses: circuit.z_spider(0.0) %a, %b
```

Result field names determine the order in the result list (matching `HasResults` iteration = struct field declaration order).

### Backward compatibility

The old format with `{result:name}` and `{result:type}` must still be accepted during a transition period. The derive detects:
- **New format**: no `ResultValue` field references with `:name` or `:type` → results handled generically
- **Legacy format**: `ResultValue` fields referenced with `:name`/`:type` → results handled by format string (single-result only, as today)

This allows gradual migration without breaking existing dialects.

### Parser architecture

```
Statement parser (generic):
  1. Try parse: result_name_list '='          → Vec<&str> (result names)
  2. Delegate to dialect parser (format string) → dialect AST
  3. Try parse: '->' type_list                 → Vec<Type> (result types)
  4. Assign names + types to ResultValue fields by position
```

The dialect parser only sees tokens between `=` and `->` (or end of statement).

### EmitIR changes

`ResultValue::emit` currently hardcodes `ResolutionInfo::Result(0)`. With multi-result:
- Each `ResultValue` field gets its index from `HasResults` iteration order
- The emit codegen passes the index: `ResolutionInfo::Result(field_index)`
- The statement parser provides (name, type) pairs by position

### PrettyPrint changes

The statement-level printer (not the dialect format printer) handles:
- Print `%name1, %name2 = ` prefix for results
- Delegate to dialect `PrettyPrint` for the body
- Print `-> Type1, Type2` suffix for result types

Dialect `PrettyPrint` only prints the operation body (keyword + operands).

## Alternatives

### Alternative A: Teach the format DSL about statement structure (original RFC design)

Keep results in the format string but add statement structure awareness:

```rust
#[chumsky(format = "{ctrl_out:name}, {tgt_out:name} = {.cnot} {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}")]
```

**Rejected because:** Couples generic statement structure to dialect format strings. Every dialect author must manually write the result/type boilerplate. MLIR's separation is cleaner and more principled.

### Alternative B: Keep results in format, add `{results}` group syntax

```rust
#[chumsky(format = "{results:name} = {.cnot} {ctrl}, {tgt} -> {results:type}")]
```

Where `{results:name}` expands to all `ResultValue` fields comma-separated.

**Rejected because:** Still couples statement structure to the format DSL. The `{results}` directive would be pure boilerplate that every format string must include.

### Recommendation

The MLIR-aligned approach (main design) is best because:
- Generic structure parsed once, correctly, at the statement level
- Format DSL becomes simpler (only operation body)
- Multi-result works automatically (no format string changes)
- Consistent with MLIR conventions that Kirin already follows
- Less boilerplate in format strings

## Crate impact matrix

| Crate | Impact | Changes |
|-------|--------|---------|
| `kirin-chumsky` | **Primary** | Statement parser: add result list + type list parsing around dialect body |
| `kirin-chumsky` | **Primary** | EmitIR: ResultValue emit with index, statement-level result assignment |
| `kirin-derive-chumsky` | **Primary** | Codegen: remove `{result:name}`/`{result:type}` from generated parsers, move to statement level |
| `kirin-derive-chumsky` | **Primary** | Validation: new-format detection, legacy-format backward compat |
| `kirin-prettyless` | **Primary** | Statement-level result/type printing (extract from dialect PrettyPrint) |
| `kirin-derive-chumsky` | **Secondary** | PrettyPrint codegen: remove result printing from format, delegate to statement level |
| All dialect crates | **Migration** | Remove `{result:name}` and `{result:type}` from format strings |

## Migration path

### Phase 1: Add generic result parsing (non-breaking)
- Statement parser learns to parse `%a, %b = ... -> T, T` generically
- Detect new-format vs legacy-format in derive
- New format strings (without `{result:name}`) use generic parsing
- Old format strings continue to work unchanged

### Phase 2: Migrate existing dialects
- Update all dialect format strings to remove `{result:name}` and `{result:type}`
- Each dialect becomes simpler
- Can be done crate-by-crate

### Phase 3: Remove legacy format support
- After all dialects migrated, remove `{result:name}`/`{result:type}` handling from derive codegen
- Clean up validation

## Validation

- **Snapshot tests**: Generated parser/AST/emit code for 0-result, 1-result, 2-result, N-result structs
- **Roundtrip test**: Parse `%a, %b = op %x, %y -> T, T` → print → re-parse
- **Legacy compat test**: Old format strings still work during Phase 1
- **Migration test**: Same dialect with old and new format produces identical IR
- **Existing tests**: All current tests must pass unchanged at every phase

## Open Questions

1. **Result-type elision**: Should `-> Type` be optional when all results use auto-placeholder? MLIR's `InferTypeOpInterface` allows this. For Kirin, results with `#[kirin(type = expr)]` already infer types — the `->` suffix could be omitted.

2. **Type suffix syntax**: Currently `-> T1, T2`. Should it change to `: (T1, T2)` to match MLIR's generic format more closely? Or keep `->` for Kirin consistency?

3. **Zero-result statements**: Currently format strings for zero-result ops (like `ZSpider`) have no `{result:name}` or `{result:type}`. In the new design, nothing changes for these — the statement parser simply skips the result/type prefix/suffix.

4. **Interaction with graph bodies**: Edge statements in UnGraph use `edge %w = op -> T;` prefix. The `edge` keyword is already parsed at the graph statement level. The result list should compose with it: `edge %w1, %w2 = op -> T, T;`.

## Reference Implementation Plan

1. **Statement-level result parsing** in `kirin-chumsky/src/parsers/stmt.rs` — parse `result_list '='` prefix and `'->' type_list` suffix generically
2. **Derive detection** in `kirin-derive-chumsky/src/validation.rs` — detect new-format (no result fields in format) vs legacy
3. **Parser codegen** in `chain.rs` — for new-format, don't include result parsing in dialect chain; statement parser handles it
4. **EmitIR** — pass result index to `ResultValue::emit`; statement-level result assignment
5. **PrettyPrint** — extract result/type printing from dialect format to statement-level printer
6. **Snapshot + roundtrip tests**
7. **Migrate toy-qc** — unify CnotCtrl/CnotTgt into single CNOT
8. **Migrate existing dialects** (Phase 2) — simplify all format strings
