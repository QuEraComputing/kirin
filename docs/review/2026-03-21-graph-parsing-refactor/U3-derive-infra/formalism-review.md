# U3: Derive Infrastructure -- Formalism Review

## Findings

### [P2] [likely] `Layout` trait's 4-parameter associated type family could be simplified via a single HKT-like pattern -- ir/layout.rs:34

The `Layout` trait has four associated types (`StatementExtra`, `ExtraGlobalAttrs`, `ExtraStatementAttrs`, `ExtraFieldAttrs`), each with different parse-level bounds (`FromDeriveInput`, `FromVariant`, `FromField`). This is a product of four independent extension points, which is the correct formalization for a 4-level hierarchy. However, the interaction between `extra_statement_attrs_from_input` (which bridges the global/statement level gap) and the `#[darling(allow_unknown_fields)]` workaround for shared namespaces suggests that the level distinction between "global" and "statement" attributes is leaky.

From a categorical perspective, `Layout` is a *profunctor* mapping parse-source levels to attribute types. The `extra_statement_attrs_from_input` method is a natural transformation between the global and statement levels. The current design is correct but the bridge method could be made more principled.

**Alternative formalisms:**

| Approach | Extension points | Shared-namespace issue | Complexity |
|----------|-----------------|----------------------|------------|
| Current 4-type Layout | 4 independent | Requires bridge method | Moderate |
| Nested `Layout<Level>` with level phantom | 1 generic | Dispatched by level | Higher (GAT-like) |
| Single `ExtraAttrs` with level enum tag | 1 | Resolved at runtime | Lower but less type-safe |

**Suggested action:** The current design is adequate. The bridge method should be better documented with an example of why `allow_unknown_fields` is needed. No structural change required.

**References:** Gibbons, "Design Patterns as Higher-Order Datatype-Generic Programs" (profunctor-based extension points).

### [P2] [uncertain] `Template` trait uses `Vec<TokenStream>` return instead of a structured document algebra -- template/mod.rs:52

The `Template` trait returns `Vec<TokenStream>`, which is an unstructured flat list. A more principled approach would be a *document algebra* (in the style of Wadler's "A prettier printer") where templates compose via algebraic operations (horizontal/vertical concatenation, nesting). However, since proc-macro output is inherently a flat token stream concatenated at the end, the current `Vec<TokenStream>` is operationally sufficient. The `CompositeTemplate` handles composition via flattening.

**Alternative formalisms:**

| Approach | Composability | Type safety | Complexity |
|----------|--------------|-------------|------------|
| `Vec<TokenStream>` (current) | Flat concat | Untyped | Low |
| Free monad over template operations | Full algebra | Typed | High (overkill) |
| Builder pattern with `impl Add` | Pairwise | Moderate | Moderate |

**Suggested action:** No change needed. The flat `Vec<TokenStream>` is pragmatically correct for proc-macro generation where the output is just concatenated. The structured algebra would add complexity without benefit.

**References:** Wadler, "A prettier printer" (document algebras); Swierstra, "Data types a la carte" (free monad composition).

## Strengths

- The `Input<L>` / `Statement<L>` / `FieldInfo<L>` three-level hierarchy is a clean functor over `Layout`, enabling different derives to attach custom metadata at each level without modifying the core IR model.
- The `VariantRef<'a, L>` enum distinguishing wrapper from regular variants provides a clean pattern-match interface that avoids boolean flags.
- Factory methods on `TraitImplTemplate` (`bool_property`, `field_iter`, `marker`) form a principled combinator library for the most common derive patterns, reducing boilerplate while preserving full customizability via `MethodSpec`.
