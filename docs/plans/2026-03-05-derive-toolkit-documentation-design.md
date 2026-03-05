# kirin-derive-toolkit Rustdoc Documentation Design

**Date:** 2026-03-05
**Audience:** Both internal team members and external contributors
**Format:** Rustdoc (in-code documentation)
**Approach:** Top-down narrative with API examples on key types

## Scope

Document `kirin-derive-toolkit` so downstream derive macro developers can understand the architecture
and use the API without reading all 54 source files. All documentation lives in Rust doc comments —
no standalone markdown guide.

## Architecture Summary

The toolkit follows a pipeline:

```
DeriveInput → Input<L> (IR parsing) → Scan (collect metadata) → Emit (generate code) → TokenStream
```

## Deliverables

### 1. Crate-Level Doc (`lib.rs`)

- One-sentence purpose
- ASCII architecture diagram showing the pipeline
- Module map organized by layer:
  - **IR layer**: `ir` (parsed representation), `ir::fields` (field classification algebra)
  - **Visitor layer**: `scan` (input traversal), `emit` (output generation)
  - **Generator layer**: `generators::builder`, `generators::field`, `generators::property`, `generators::marker`, `generators::stage_info`
  - **Code-gen layer**: `tokens` (typed code blocks), `codegen` (constructor/generics/binding utilities)
  - **Support**: `context` (pre-computed state), `derive` (metadata), `stage` (stage parsing), `misc` (utilities)
- Brief note on `Layout` extensibility (`StandardLayout` vs custom)

### 2. Module-Level Docs

| Module | Approx lines | Content |
|--------|-------------|---------|
| `ir` | 15 | `Input<L>` → `Statement<L>` → `FieldInfo<L>` hierarchy, `Layout` trait, example parsing |
| `ir::fields` | 10 | Field classification algebra, all categories, `Collection` wrapping |
| `scan` | 10 | Visitor pattern, 13 hook methods, override pattern |
| `emit` | 10 | Visitor pattern, 13 hook methods, code generation pattern |
| `generators` | 8 | Overview of 5 pre-built generators, composition pattern |
| `tokens` | 10 | Typed code blocks vs raw `quote!`, key types listed |
| `codegen` | 6 | Utility overview |
| `context` | 6 | `DeriveContext` / `StatementContext` purpose |
| `derive`, `stage`, `misc` | 2-3 each | Brief support utility descriptions |

### 3. Key Type/Trait Docs with API Examples

**Must-document:**

| Type/Trait | Example Shows |
|-----------|---------------|
| `Layout` trait | `StandardLayout` vs custom layout definition |
| `Input<L>` | Parsing from `DeriveInput`, accessing statements |
| `Statement<L>` | Iterating fields, checking `#[wraps]` |
| `FieldInfo<L>` / `FieldCategory` | Matching on category, `ssa_type()` |
| `Scan<'ir, L>` | Overriding `scan_statement` to collect names |
| `Emit<'ir, L>` | Overriding `emit_statement` to generate match arms |
| `TraitImpl` | Building trait impl with methods and associated types |
| `MatchExpr` / `Pattern` | Building a match over enum variants |

**Nice-to-have:**

| Type/Trait | Example Shows |
|-----------|---------------|
| `DeriveContext<'ir, L>` | Accessing pre-built patterns |
| `GenerateBuilder<'ir, L>` | Composing generators |
| `ConstructorBuilder` | Building a `new()` function |
| `DeriveBuilder`, `DeriveFieldIter`, `DeriveProperty` | Usage of pre-built generators |

## Non-Goals

- No standalone markdown guide (all in rustdoc)
- No end-to-end tutorial (just API examples per type)
- No documentation of internal/private implementation details
