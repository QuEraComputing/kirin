# U3: Derive Infrastructure -- Ergonomics/DX Review

## Toy Scenario

I am a derive macro author adding support for a new IR body type (say `HyperGraph`). How many files do I touch?

1. **kirin-derive-toolkit/src/ir/fields/**: Add `FieldCategory::HyperGraph`, `FieldData::HyperGraph`, classification logic. At least 2-3 files.
2. **kirin-derive-toolkit/src/template/method_pattern/field_collection.rs**: Add `FieldIterKind::HyperGraphs`.
3. **kirin-derive-ir/src/generate.rs**: Add `HAS_HYPERGRAPHS` and `HAS_HYPERGRAPHS_MUT` configs (4 const entries + 2 array additions).
4. **kirin-ir/src/language.rs**: Add `HasHypergraphs<'a>` and `HasHypergraphsMut<'a>` traits.
5. **kirin-derive-chumsky/src/field_kind.rs**: Add `FieldCategory::HyperGraph` arms in `ast_type`, `parser_expr`, `print_expr`.
6. **kirin-derive-chumsky/src/validation.rs**: Add HyperGraph to body projection validation.

That is a minimum of **6 crates, ~10 files** for a single new body type. The template system makes the code generation itself clean, but the surface area of required changes is large.

## Findings

### [P2] [confirmed] Adding a new FieldCategory requires touching ~10 files across 4+ crates -- generate.rs, field_kind.rs, language.rs

Each new IR body type (DiGraph, UnGraph, now HyperGraph hypothetically) requires: trait pair in kirin-ir, const configs in kirin-derive-ir, field_kind arms in kirin-derive-chumsky, classification in kirin-derive-toolkit. This is a registry problem. A trait-per-category approach is intentional for performance but creates high coordination cost for contributors adding categories.

### [P1] [confirmed] 14 field iterator configs + 5 property configs = 19 const declarations for the Dialect derive -- generate.rs:22-184

The `generate.rs` file is 70% const declarations. Each `FieldIterConfig` repeats the same pattern: kind, mutability, trait name, matching type, method name, iterator type name. These could be generated from a single registry table. Currently, a typo in any of these strings produces a compile error in *user code*, not in the derive crate -- hard to trace back.

### [P3] [uncertain] Layout trait has 4 associated types all defaulting to () -- layout.rs:34-48

For most derives, `StandardLayout` suffices. But when you need custom attributes (like `ChumskyLayout`), you must implement all 4 associated types even if you only need one. A builder or partial-override pattern would reduce boilerplate for custom layouts.

## Concept Budget Table

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| Input<L> / Data / Statement | ir/input.rs | Med |
| Layout / StandardLayout | ir/layout.rs | Med |
| Template trait | template/mod.rs | Low |
| TraitImplTemplate + factory methods | template/trait_impl.rs | Med |
| MethodSpec / MethodPattern | template/method_pattern/ | High |
| DeriveContext / StatementContext | context/mod.rs | Med |
| FieldCategory / FieldInfo / FieldData | ir/fields/ | High |
| CompositeTemplate / TemplateBuilder | template/composite.rs, builder.rs | Low |

## Lifetime Complexity

(i) **Hidden by derive**: Users of `#[derive(Dialect)]` never see any of this infrastructure.
(ii) **Visible necessary**: `DeriveContext<'ir, L>` borrows the `Input` -- required for reference-based codegen.
(iii) **Visible avoidable**: None.

## Strengths

- `ir.compose().add(template).build()` is a genuinely clean API. Adding a new template is one line.
- `TraitImplTemplate::bool_property()` and `::field_iter()` factory methods eliminate most boilerplate for standard patterns.
- `BuilderTemplate::new()` handles constructor generation with zero configuration.
- `DeriveContext` pre-computation avoids repeated work across templates.
- Error accumulation via `darling::Error::accumulator()` in `CompositeTemplate` reports all errors, not just the first.
