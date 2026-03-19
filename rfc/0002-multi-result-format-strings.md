# RFC 0002: Multi-Result Statement Format Strings

- **Status**: Draft
- **Authors**: roger, claude
- **Created**: 2026-03-19

## Summary

Extend the `#[chumsky(format = "...")]` DSL to support operations with multiple result values. Currently, each `ResultValue` field maps to one parse/print action, but there is no way to express a comma-separated result list on the LHS of `=`. This forces multi-output operations (like quantum CNOT or MLIR-style ops returning multiple values) to be split into separate single-result operations.

## Motivation

Quantum circuits, MLIR-style operations, and other domains naturally have operations that produce multiple results:

```
// Quantum CNOT: 2 qubits in, 2 qubits out
%ctrl_out, %tgt_out = circuit.cnot %ctrl, %tgt -> Qubit, Qubit;

// MLIR-style divmod: returns both quotient and remainder
%q, %r = arith.divmod %a, %b -> i64, i64;
```

Today, the format DSL cannot express this. A struct with two `ResultValue` fields:

```rust
pub struct CNOT {
    pub ctrl: SSAValue,
    pub tgt: SSAValue,
    pub ctrl_out: ResultValue,
    pub tgt_out: ResultValue,
}
```

Has no valid format string that parses `%ctrl_out, %tgt_out = cnot %ctrl, %tgt -> Qubit, Qubit`. The workaround is splitting into two operations (`CnotCtrl`, `CnotTgt`), which misrepresents the semantics — CNOT is an atomic 2-in-2-out gate.

### Current limitations by layer

| Layer | Limitation |
|-------|-----------|
| Format DSL (`format.rs`) | No syntax for comma-separated result lists |
| Parser combinators (`values.rs`) | No `result_value_list()` parser |
| AST construction (`chain.rs`) | One field → one parse action, no accumulation |
| EmitIR (`values.rs`) | `ResolutionInfo::Result(0)` hardcoded, no multi-index tracking |
| Pretty printer | Single result per field, no comma-separated output |

## Design

### Format string syntax

Introduce the concept of **result groups** — consecutive `ResultValue` fields separated by commas on the LHS of `=`:

```rust
#[chumsky(format = "{ctrl_out:name}, {tgt_out:name} = {.cnot} {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}")]
pub struct CNOT {
    pub ctrl: SSAValue,
    pub tgt: SSAValue,
    pub ctrl_out: ResultValue,
    pub tgt_out: ResultValue,
}
```

This is the same syntax users already try to write. The DSL parser (`format.rs`) already parses this correctly — the `{ctrl_out:name}` and `{tgt_out:name}` are valid `FormatElement::Field` entries separated by a `FormatElement::Token(Comma)`. No format DSL changes needed.

### Statement structure awareness

The key change is teaching the codegen to understand **statement structure**. Currently, format elements are treated as a flat sequence. The proposed change partitions them into semantic regions:

```
[results] = [keyword] [operands] -> [types]
```

Where:
- **results**: All `{field:name}` occurrences for `ResultValue` fields before the `=` token
- **keyword**: The `{.keyword}` element
- **operands**: All `{field}` or `{field:name}` occurrences for non-result fields
- **types**: All `{field:type}` occurrences after `->` token

### Detection

A format string is multi-result when:
1. It contains 2+ `ResultValue` fields with `:name` occurrences before `=`
2. The fields are separated by `,` tokens in the format

The validation layer (`validation.rs`) detects this by scanning for the `=` token and counting result-name occurrences before it.

### Parser generation changes

**`parsers/values.rs`**: Add a `result_value_list()` parser:

```rust
pub fn result_value_list<'t, I, T>() -> impl Parser<'t, I, Vec<ResultValue<'t, T>>>
where ...
{
    result_value().separated_by(just(Token::Comma)).at_least(1).collect()
}
```

**`codegen/parser/chain.rs`**: When a multi-result format is detected, generate:
1. Parse the result name list as a `Vec<ResultValue>` (comma-separated)
2. Destructure into individual named variables
3. Continue parsing the RHS as before
4. Parse the type list and assign types to results by position

### AST generation changes

**`codegen/ast/definition.rs`**: Multi-result structs get a `Vec<ResultValue<'t, TypeOutput>>` for the result group instead of individual fields. Or keep individual fields but generate destructuring code from the parsed vector.

### EmitIR changes

**`ast/values.rs`**: `ResultValue::emit` currently uses `ResolutionInfo::Result(0)`. For multi-result, each result needs its correct index:

```rust
// Result 0 (ctrl_out)
let ctrl_out_ssa = ctx.stage.ssa(name, ty, BuilderSSAKind::Unresolved(
    ResolutionInfo::Result(0)
));

// Result 1 (tgt_out)
let tgt_out_ssa = ctx.stage.ssa(name, ty, BuilderSSAKind::Unresolved(
    ResolutionInfo::Result(1)
));
```

The index is determined by the field's position among all `ResultValue` fields in the struct (matching `HasResults` iteration order).

### Pretty printer changes

**`codegen/pretty_print/generate.rs`**: For multi-result, print all result names comma-separated before `=`, and all result types comma-separated after `->`.

## Alternatives

### Alternative A: Tuple result field

Instead of multiple `ResultValue` fields, use a single `Vec<ResultValue>` or tuple:

```rust
pub struct CNOT {
    pub ctrl: SSAValue,
    pub tgt: SSAValue,
    pub results: (ResultValue, ResultValue),
}
```

**Pros:** One field, simpler format: `{results:name} = {.cnot} {ctrl}, {tgt} -> {results:type}`.
**Cons:** Loses named access (`self.ctrl_out`), requires destructuring everywhere, breaks `HasResults` iteration.

### Alternative B: Statement-level result annotation

Add a struct-level attribute instead of format string changes:

```rust
#[chumsky(results = "ctrl_out, tgt_out")]
#[chumsky(format = "{.cnot} {ctrl}, {tgt}")]
pub struct CNOT { ... }
```

**Pros:** Clean separation of result declaration from format.
**Cons:** Duplicates information (field names appear in both the struct and the attribute), non-obvious interaction with the format string.

### Recommendation

The format-string-based approach (the main design) is best because:
- Users already try to write it this way naturally
- The format DSL already parses it correctly
- Changes are localized to codegen + emit, not the DSL grammar
- Named fields preserved (`self.ctrl_out`, `self.tgt_out`)

## Crate impact matrix

| Crate | Impact | Changes |
|-------|--------|---------|
| `kirin-derive-chumsky` | **Primary** | Codegen: parser chain, AST, emit_ir, pretty_print |
| `kirin-derive-chumsky` | **Primary** | Validation: multi-result detection |
| `kirin-chumsky` | **Secondary** | Add `result_value_list()` parser; update EmitIR for ResultValue with index |
| `kirin-prettyless` | **Minimal** | PrettyPrint codegen for comma-separated results |
| Dialect crates | **None** | Existing single-result ops unchanged |

## Validation

- **Snapshot tests**: Generated parser/AST/emit/print code for a 2-result struct
- **Roundtrip test**: Parse `%a, %b = op %x, %y -> T, T` → print → re-parse
- **Compile-fail test**: 2 results without types should error
- **Existing tests**: All current single-result tests must pass unchanged

## Open Questions

1. **Maximum result count?** Should we cap at some limit or allow arbitrary N? Arbitrary seems fine — `Vec<ResultValue>` is already the IR representation.

2. **Mixed result/non-result before `=`?** Should `{name}, {result:name} = ...` be valid (mixing a non-result field name with a result name)? Probably not — everything before `=` should be a result.

3. **Result ordering**: Should result index match field declaration order in the struct, or format string order? Struct declaration order is more predictable and matches `HasResults` iteration.

4. **Interaction with `#[kirin(type = expr)]`**: When a result has an explicit type expression like `#[kirin(type = value.type_of())]`, the `:type` in the format string is the parsed type. These should compose — the format `:type` is the text representation, the `#[kirin(type)]` is the IR type.

## Reference Implementation Plan

1. **Add `result_value_list()` parser** in `kirin-chumsky/src/parsers/values.rs`
2. **Add multi-result detection** in `kirin-derive-chumsky/src/validation.rs`
3. **Update parser chain codegen** in `chain.rs` to generate list parse + destructure
4. **Update EmitIR** for ResultValue to accept a result index parameter
5. **Update PrettyPrint codegen** for comma-separated result rendering
6. **Add snapshot + roundtrip tests**
7. **Migrate toy-qc CNOT** from CnotCtrl/CnotTgt back to single CNOT with 2 results
