+++
rfc = "0004"
title = "Value Format Specifier"
status = "Draft"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-09T04:05:31.660638Z"
last_updated = "2026-02-09T22:27:50Z"
dependencies = ["0002"]
+++

# RFC 0004: Value Format Specifier

## Summary

Add a new chumsky format interpolation option, `{field:value}`, for compile-time
value fields. This gives dialect authors an explicit way to request
"value-only" surface syntax independent of `{field}` default behavior and
unblocks cleaner constant formats in arithmetic-heavy dialects. When
`{field:value}` is used for a compile-time value field `field`, the same format
string must also contain `{field:type}`. Other type selectors (for example
`{result:type}`) do not satisfy this requirement. Cross-field type consistency
(for example `{value:type}` vs `{result:type}`) is not validated by format
validation and is deferred to a future verification RFC.

## Motivation

- Problem: Current format options only include default, `:name`, and `:type`.
  There is no explicit "value payload" option for compile-time value fields.
- Why now: RFC 0002 references `{field:value}` to avoid heuristic value parsing
  limitations in arithmetic constants.
- Stakeholders:
  - `kirin-chumsky-format` and derive maintainers
  - dialect authors with constants (`kirin-constant`, `kirin-arith`)
  - parser/printer roundtrip users

## Goals

- Add `{field:value}` syntax to format strings.
- Restrict `:value` to compile-time value fields (`FieldKind::Value`).
- Require per-field co-occurrence: each compile-time value field that uses
  `:value` must also use `:type` on that same field.
- Keep parser/printer generation deterministic and roundtrip-safe.
- Preserve parse-time information for both value payload and type annotation
  with field-local parse/emit behavior.
- Defer cross-field type consistency checks to a future verification RFC.

## Non-goals

- General expression parsing inside format strings.
- Replacing `HasParser` or `PrettyPrint` traits.
- Introducing multiple independent format grammars.

## Guide-level Explanation

Dialect authors can choose whether a value field uses default formatting or
value-only formatting:

```rust
#[chumsky(format = "{result:name} = constant {value:value} : {value:type} -> {result:type}")]
struct Constant<T, Ty> {
    value: T,
    #[kirin(type = value.type_of())]
    result: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<Ty>,
}
```

Meaning of options:

- `{field}`: default behavior for that field type.
- `{field:name}`: only valid for SSA/Result fields.
- `{field:type}`:
  - for SSA/Result fields: print/parse the SSA type component.
  - for compile-time value fields: print/parse the value field's explicit type
    component.
- `{field:value}`: only valid for compile-time value fields.

Validation error example:

- Using `{lhs:value}` where `lhs: SSAValue` is rejected.
- Using `{value:value}` without `{value:type}` in the same format string is
  rejected (and `{result:type}` does not count).

## Reference-level Explanation

### API and syntax changes

- Extend `FormatOption` in `crates/kirin-chumsky-format/src/format.rs`:

```rust
pub enum FormatOption {
    Name,
    Type,
    Value,
    Default,
}
```

- Extend format parser to recognize `:value`.
- Extend validation visitor:
  - `:value` is allowed only on `FieldKind::Value`.
  - `:type` is allowed on `FieldKind::Value` in addition to SSA/Result fields.
  - for each value field `f`, if `{f:value}` exists then `{f:type}` must also
    exist.
- Extend parser and pretty-printer codegen:
  - parser: value-field occurrence with `:value` uses value-specific parse path
    while `{field:type}` parses the corresponding value-type component
  - printer: value-field occurrence with `:value` uses value-specific print path
    and `{field:type}` prints the corresponding value-type component

### Semantics and invariants

- `:value` does not change statement structure; it only changes how one field is
  parsed/printed.
- Validation must fail fast when format options do not match field category.
- For every value field `f`, `:value` requires `:type` on the same field `f`;
  other fields' type selectors are irrelevant to this check.
- Parser output retains value and type information separately per field.
- Emit/lowering is field-local: selectors on field `f` only affect field `f`;
  emit/lowering must not infer one field from another.
- Cross-field consistency checks (including `{value:type}` vs `{result:type}`
  mismatches) are deferred to a future verification RFC.
- Roundtrip target remains `print -> parse -> print` under the same format.

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-chumsky-format` | parse/validate/generate support for `:value` | format parser + validation + generation snapshots |
| `kirin-chumsky-derive` | derive snapshots for value-format statements | snapshot tests |
| `kirin-chumsky` | optional runtime parser helper for value fields | parser combinator tests |
| `kirin-prettyless` | optional value-only printing helper trait/path | trait + impl tests |
| `kirin-constant` | demonstration and regression coverage | roundtrip tests |

## Drawbacks

- Adds another format option and validation branch.
- Emit/lowering must clearly document field-local typed value construction rules.
- Cross-field mismatch diagnostics depend on a future verification framework RFC.
- May expose latent assumptions in existing value `HasParser` implementations.

## Rationale and Alternatives

### Proposed approach rationale

- Keeps format language small and explicit.
- Aligns with existing option model (`name`, `type`) instead of inventing a
  separate constant-format DSL.
- Guarantees format text contains enough information to recover both value and
  type without relying on parser-time heuristics.

### Alternative A

- Description: Keep using `{field}` with value-type-specific pretty/parse logic.
- Pros: zero new syntax.
- Cons: no explicit contract; behavior hidden in value type impls.
- Reason not chosen: ambiguity and poor readability in RFC-scale dialect docs.

### Alternative B

- Description: Introduce a separate constant mini-language, not format options.
- Pros: maximal control over literal syntax.
- Cons: duplicates parser infrastructure and fragments tooling.
- Reason not chosen: too heavy for the problem size.

## Prior Art

- Python/Rust-like format option style (`field:option`) for concise annotation.
- Existing Kirin `:name`/`:type` behavior and validation architecture.

## Backward Compatibility and Migration

- Breaking changes: none for existing format strings.
- Migration steps:
  1. opt in by replacing `{value}` with `{value:value}` where explicitness is desired
  2. for each `{field:value}`, add matching `{field:type}` in the same format string
  3. update snapshots
- Compatibility strategy: existing options and default behavior remain valid.

## How to Teach This

- Update `design/text-format.md` option list with `:value`.
- Add one end-to-end constant example in docs and derive tests.
- Document option compatibility matrix by field category.

## Reference Implementation Plan

1. Add `FormatOption::Value` parsing in `format.rs`.
2. Extend validation visitor for field-kind checks and `:value` + `:type`
   co-occurrence requirements on the same value field.
3. Update parser/pretty code generation for value option path and retained type
   information.
4. Add derive + snapshot tests covering success and failure cases.

### Acceptance Criteria

- [ ] `{field:value}` parses in format strings.
- [ ] validation rejects `:value` on non-value fields.
- [ ] validation rejects `{field:value}` when `{field:type}` is missing for the
  same field.
- [ ] pretty-print generation handles `:value`.
- [ ] roundtrip tests cover at least one constant statement using `:value`
  together with matching same-field `:type`.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - adopt in `kirin-arith` constants once available
  - define canonical field-local emit/lowering mapping from parsed
    `{field:value}` + `{field:type}` pairs
  - write a verification framework RFC (traits + pass model) that defines and
    enforces cross-field type consistency, including `{value:type}` vs
    `{result:type}` mismatch rejection

## Unresolved Questions

- None for this RFC. Cross-field type consistency between `{value:type}` and
  `{result:type}` is intentionally deferred to a future verification RFC.

## Future Possibilities

- Additional options for numeric literals (`:hex`, `:bin`) if needed.
- Parser diagnostics that suggest `:value` when default formatting is ambiguous.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-09T04:05:31.660638Z | RFC created from template |
| 2026-02-09 | Replaced template with concrete `{field:value}` proposal and phase model |
| 2026-02-09 | Decision refined: `:type` pairing is per value field (same-field), not any global type selector |
| 2026-02-09 | Decision: `{field:value}` must pair with same-field `{field:type}` (`{result:type}` does not satisfy) |
| 2026-02-09 | Resolved open questions: parse/emit are field-local; `{value:type}` vs `{result:type}` mismatches are deferred to a future verification RFC |
