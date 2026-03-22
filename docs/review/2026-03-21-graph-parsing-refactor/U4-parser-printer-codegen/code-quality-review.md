# U4: Parser/Printer Codegen (kirin-derive-chumsky + kirin-derive-prettyless) -- Code Quality Review

## Clippy / Lint Findings

No `#[allow]` or `#[expect]` annotations found in either crate. Clean from a lint-suppression perspective.

## Duplication Findings

### [P2] [likely] field_kind.rs ast_type() match arms -- field_kind.rs:56-109
The function has a 10-arm match on `FieldCategory` where several arms follow the same pattern: `quote! { #crate_path::TypeName<'t, #type_output> }`. The DiGraph and UnGraph arms produce nearly identical token streams. Suggested abstraction: A helper `graph_ast_type(kind_name, crate_path, type_output, language_output)`. Lines saved: ~10. Low impact.

### [P2] [likely] pretty_print/statement.rs vs parser/chain.rs -- parallel category dispatching
Both files (518 and 615 lines respectively) switch on `FieldCategory` to generate code for each field kind. The pretty-print side generates `doc.print_*` calls; the parser side generates parser combinator chains. While the output is fundamentally different, both follow the same field-iteration-then-category-dispatch structure. Not easily unified due to different output domains, but documenting the parallel structure would help maintainers.

### [P2] [confirmed] validation.rs HashSet bookkeeping (676 lines)
The `ValidationVisitor` tracks 7 different sets/maps (`occurrences`, `default_occurrences`, `referenced_fields`, `name_occurrences`, `result_name_occurrences`, `body_projections`, `errors`). While correct, the struct has high field count. Consider grouping related tracking into sub-structs: `OccurrenceTracker { defaults, references, names, result_names }`.

## Rust Best Practices

### [P1] [likely] chain.rs at 615 lines -- single function risk
Parser chain generation handles all field categories, collection wrapping (Single/Vec/Option), format projections, and recursive parser threading in a single module. If the main generation function exceeds ~100 lines, it should be decomposed into per-category helpers. Verify the longest function length.

### [P3] [uncertain] Unused parameters in field_kind.rs:48-54
`ast_type()` takes `_ast_name` and `_type_params` parameters that are prefixed with `_`, indicating they are unused. If these were once needed and are now dead, removing them simplifies the API. If reserved for future use, add a doc comment explaining the intent.

## Strengths

- Format string system (`Format`, `FormatElement`, `FormatOption`, `BodyProjection`) is expressive and well-validated before codegen.
- Validation produces clear, actionable error messages with span information pointing to the derive attribute.
- kirin-derive-prettyless is remarkably compact (2 files) compared to the parser derive, reflecting good abstraction of the simpler pretty-print problem.
- The codegen split into `ast/`, `emit_ir/`, `parser/`, `pretty_print/` sub-modules keeps concerns well-separated.
