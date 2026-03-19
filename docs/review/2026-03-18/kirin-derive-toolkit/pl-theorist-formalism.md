# PL Theorist — Formalism Review: kirin-derive-toolkit

## Abstraction Composability

### Template system: composable code generation algebra

The template system (`template/mod.rs:52-66`) defines a `Template<L>` trait:

```rust
trait Template<L: Layout> {
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>;
}
```

Templates compose via the `TemplateBuilder` (which collects templates and concatenates their outputs) and `CompositeTemplate` (which aggregates multiple templates). The composition is monoidal — each template produces a `Vec<TokenStream>`, and composition concatenates these vectors. The identity element is an empty template (produces `vec![]`), and associativity holds trivially.

The closure blanket impl (`template/mod.rs:58-66`) allows ad-hoc templates without defining new types. This provides the **open extension** property: users can inject custom logic without modifying the framework.

### TraitImplTemplate: method pattern algebra

`TraitImplTemplate<L>` (`template/trait_impl.rs:19-31`) aggregates `MethodSpec<L>` entries. Each `MethodSpec` contains a `MethodPattern<L>` that defines per-struct and per-variant behavior:

```
TraitImplTemplate = trait_path + [MethodSpec]
MethodSpec = name + MethodPattern
MethodPattern = for_struct(ctx, stmt) -> TokenStream
              | for_variant(ctx, stmt) -> TokenStream
```

The method patterns compose independently — each method has its own pattern, and multiple methods can be added to the same trait impl. The factory methods (`bool_property`, `field_iter`, `marker`) provide pre-built patterns for common cases.

This is a **tagless final** encoding of code generation: instead of building an AST of "what to generate" and then interpreting it, the templates directly produce `TokenStream` fragments. The advantage is simplicity and directness; the disadvantage is that optimization/analysis of the generated code requires post-hoc processing.

### Layout trait: extensible attribute parsing

The `Layout` trait (`ir/layout.rs`) parameterizes the IR model over derive-specific attributes:

```rust
trait Layout {
    type StatementExtra;
    type ExtraGlobalAttrs: FromDeriveInput;
    type ExtraStatementAttrs;
    type ExtraFieldAttrs;
}
```

This is a **type family** pattern — each derive macro defines its own `Layout` impl with the attribute types it needs. `StandardLayout` provides `()` for all extras, serving as the base case.

The composition is clean: `kirin-derive-chumsky` defines `ChumskyLayout` with `ChumskyGlobalAttrs`, `ChumskyStatementAttrs`, and `ChumskyFieldAttrs`. `kirin-derive-interpreter` would define its own layout (e.g., `EvalCallLayout`). Layouts are independent — different derive macros can parse different attributes from the same source type.

The `extra_statement_attrs_from_input` method (`ir/layout.rs`) handles the global-vs-variant attribute namespace collision with a lenient parser (`#[darling(allow_unknown_fields)]`). This is a pragmatic solution to the ambiguity problem: when `#[chumsky(...)]` appears at both the type and variant level, the lenient parser at the type level ignores variant-specific fields.

### Input parsing: syn -> IR model

`Input<L>` (`ir/input.rs:27-38`) transforms `syn::DeriveInput` into a structured IR:

```
syn::DeriveInput -> Input<L> { name, generics, attrs, extra_attrs, data: Data<L> }
Data<L> = Struct(Statement<L>) | Enum([Statement<L>])
Statement<L> = name + fields: [FieldInfo] + wraps: Option<Wrapper> + extra
```

This is a standard **elaboration** pass: raw syntax is transformed into a typed intermediate representation. The `Statement` type represents a single operation (struct) or variant (enum), which maps to Kirin's dialect definition structure.

The `VariantRef` discriminated union (`ir/input.rs:190-200`) distinguishes `Wrapper` variants (delegating to inner types) from `Regular` variants (with explicit fields). This is a clean sum-type representation of the two fundamental enum patterns in Kirin.

### DeriveContext: pre-computed state

`DeriveContext` (`context/mod.rs`) pre-computes per-statement contexts (`StatementContext`) containing wrapper types, binding patterns, and field classifications. This is the **memoization** pattern: expensive computations (pattern matching, field classification) are done once during context construction and then shared across all templates.

## Literature Alignment

### Relation to macro system theory

The template system implements a form of **staged metaprogramming** (Taha & Sheard, 1997): the first stage (template construction) builds a code generator, and the second stage (template emission) produces the output code. The `DeriveContext` serves as the environment for the second stage.

The `MethodPattern` abstraction corresponds to the **strategy pattern** from design patterns, but applied to code generation. Each pattern encapsulates a different code generation strategy (bool property, field iterator, delegation, custom).

### Relation to derive macro frameworks

The toolkit occupies the same design space as `derive_more`, `derive_builder`, and `darling`, but adds domain-specific IR modeling on top. The layering (IR model -> context -> templates -> tokens) is more structured than typical derive macro implementations, which tend to mix parsing and code generation.

The `FieldCategory` / `FieldData` classification system (`ir/fields/data.rs`) maps to the MLIR operation definition spec (ODS): each field is classified as a result, argument, block, region, successor, graph edge, or auxiliary data. This is the compile-time counterpart of the runtime `Dialect` trait's capability system.

## Semantic Ambiguity

### `__`-prefixed variant filtering

`Input::from_derive_input` (`ir/input.rs:59-60`) silently filters out enum variants whose name starts with `__`. The `has_hidden_variants` flag triggers a wildcard match arm (`_ => unreachable!()`). This is an implicit convention — there is no attribute or explicit marker for hidden variants. The prefix-based convention could collide with user-chosen names, although `__` is a strong convention for "internal use".

### `wraps` detection at input level vs. variant level

The `wraps` attribute is detected differently at the input level (`input.attrs.iter().any(|f| f.path().is_ident("wraps"))` in `input.rs:66`) and at the variant level (via `Statement::from_variant`). The input-level detection passes a boolean to `Statement::from_variant`, which determines how fields are classified. This two-level detection is functional but creates a non-obvious data flow: the presence of `#[wraps]` on the type changes how every variant's fields are interpreted.

### `StatementExtra` unused in StandardLayout

`Layout::StatementExtra` is `()` in `StandardLayout` and appears to be unused in the toolkit itself. It exists for future extensibility but adds a type parameter that flows through the entire `Statement<L>` type without contributing value in the common case. This is a minor abstraction leak.

## Alternative Formalisms Considered

### 1. Template system: tagless final vs. initial algebra

**Current**: Tagless final — templates directly produce `TokenStream` (no intermediate AST).
**Alternative A**: Initial algebra — templates produce a code generation AST that is then compiled to `TokenStream`.
**Alternative B**: Monadic code generation — use a `CodeGen` monad that threads state and produces code.

| Metric | Tagless final (current) | Initial algebra | Monadic |
|--------|------------------------|-----------------|---------|
| Simplicity | High | Medium | Low |
| Optimization | None (direct output) | Possible (AST rewriting) | Possible |
| Composability | Monoidal (concatenation) | Algebraic (AST composition) | Monadic (sequencing) |
| Type safety | Medium (raw TokenStream) | High (typed AST) | High |

Tagless final is the right choice for a derive macro framework where the generated code is simple enough that optimization is unnecessary, and the directness of TokenStream production is more valuable than a typed intermediate representation.

### 2. Layout extensibility: type family vs. higher-kinded types

**Current**: Type family via associated types on `Layout`.
**Alternative A**: Higher-kinded types (if Rust supported them) — `Layout<F>` where `F: Type -> Type`.
**Alternative B**: Dynamic extension via `HashMap<String, Box<dyn Any>>`.

| Metric | Type family (current) | HKT | Dynamic |
|--------|----------------------|-----|---------|
| Type safety | Full | Full | None |
| Ergonomics | Good (associated types) | Better (abstraction) | Poor |
| Rust compatibility | Yes | No (not supported) | Yes |
| Compile-time cost | Low | Unknown | None |

Type families via associated types are the idiomatic Rust encoding of what would be HKTs in Haskell. The choice is correct for the language.

### 3. Field classification: IR model vs. attribute-driven

**Current**: Hybrid — field types are inspected (`SSAValue`, `ResultValue`, `Block`, etc.) and supplemented by attributes (`#[kirin(type = ...)]`).
**Alternative A**: Purely attribute-driven (all classification via annotations).
**Alternative B**: Purely type-driven (no attributes, all information from types).

| Metric | Hybrid (current) | Attribute-only | Type-only |
|--------|------------------|----------------|-----------|
| Boilerplate | Low (types auto-classify) | High (everything annotated) | Low |
| Correctness | Type-driven classification is sound | User-driven, error-prone | Sound but inflexible |
| Extensibility | New types need toolkit updates | New attributes only | New types only |

The hybrid approach is the right choice: it leverages Rust's type system for automatic classification while providing attributes for cases where type information alone is insufficient.

## Summary

- [P3] [confirmed] `__`-prefixed variant filtering is an implicit convention without explicit opt-in — `ir/input.rs:59-60`
- [P3] [confirmed] `StatementExtra` is always `()` in StandardLayout, adding unused complexity — `ir/layout.rs`
- [P3] [informational] Template system implements a clean monoidal composition algebra — `template/mod.rs:52-66`
- [P3] [informational] Layout type family correctly encodes derive-specific attribute extensibility — `ir/layout.rs`
- [P3] [informational] DeriveContext pre-computation follows the memoization pattern for staged metaprogramming — `context/mod.rs`
