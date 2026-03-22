# U4: Parser/Printer Codegen -- Formalism Review

## Findings

### [P1] [confirmed] `FieldCategory` enum is a closed sum that cannot be extended without modifying the core crate -- derive-toolkit field_kind, derive-chumsky field_kind.rs

`FieldCategory` is an enum with 10 variants (`Argument`, `Result`, `Block`, `Successor`, `Region`, `Symbol`, `Value`, `DiGraph`, `UnGraph`, `Signature`). Adding a new IR structural element (e.g., a `TableRegion` or `Effect` type) requires modifying `FieldCategory` in `kirin-derive-toolkit`, then updating every match arm in `kirin-derive-chumsky` (`ast_type`, `parser_expr`, `print_expr`), `kirin-derive-prettyless`, and `kirin-derive-interpreter`. This is the classic *expression problem*: adding a new case requires touching O(n) existing files.

In the current codebase, the addition of `DiGraph`, `UnGraph`, and `Signature` to the enum demonstrates the cost -- each required updating approximately 6 match exhaustiveness sites across 3 crates.

**Alternative formalisms:**

| Approach | New-case cost | New-operation cost | Type safety |
|----------|--------------|-------------------|-------------|
| Closed enum (current) | O(n) sites | O(1) add method | Full (exhaustive match) |
| Trait object `dyn FieldKind` | O(1) add impl | O(n) sites (new method) | Partial (no exhaustiveness) |
| Visitor pattern with default | O(1) if default suffices | O(1) add accept | Full with fallback |

**Suggested action:** Given that new field categories are rare (3 additions in the project's lifetime), the closed enum is acceptable. However, consider adding a `#[non_exhaustive]` attribute and providing a `_ => compile_error!("unsupported field category")` arm in generated code so that downstream derive crates get clear errors when a new category is added rather than silent breakage.

**References:** Wadler, "The Expression Problem" (1998); Krishnamurthi et al., "Synthesizing Object-Oriented and Functional Design to Promote Re-Use."

### [P2] [likely] Format string DSL lacks a formal grammar specification -- format.rs

The format string mini-language (`{field}`, `{field:name}`, `{field:type}`, `{field:ports}`, `$keyword`, `{:name}`, `{{` escape) is parsed by a chumsky parser but lacks a written BNF or PEG grammar. The grammar is defined implicitly by the parser combinator composition. This makes it difficult to reason about ambiguity (e.g., is `{body:body}` parsed as a body-projection or a field named "body" with option "body"?), and the `Token` vs `FormatElement` layering introduces a hidden precedence: escaped braces > dollar keyword > context projection > interpolation > literal tokens.

**Alternative formalisms:**

| Approach | Ambiguity analysis | Tooling | Maintainability |
|----------|-------------------|---------|-----------------|
| Implicit (parser-is-grammar, current) | Ad-hoc | None | Low |
| Explicit BNF in doc comment | Manual | lint-checkable | Moderate |
| EBNF + parser-generator | Formal (no ambiguity) | lalrpop/pest | Higher upfront |

**Suggested action:** Add an EBNF grammar in the `format.rs` module doc comment. The grammar already exists implicitly in the parser; making it explicit costs minimal effort and provides a reference for validation rule authors. Example:

```
format  ::= element*
element ::= '{{' | '}}' | '$' IDENT | '{:' projection '}' | '{' field_ref '}' | token+
field_ref ::= (IDENT | INT) (':' option)?
option  ::= 'name' | 'type' | 'ports' | 'captures' | 'args' | 'body' | 'inputs' | 'return'
projection ::= 'name'
```

**References:** Ford, "Parsing Expression Grammars" (PEG formalism for unambiguous parsing).

## Strengths

- The validation system (`ValidationVisitor`) enforces roundtrip correctness at compile time by checking that all structural projections are complete (e.g., a DiGraph with `:body` must also have `:ports` and `:captures`). This is a sound static analysis.
- The `FormatOption` enum provides a clean algebra of field projections (Name, Type, Default, Body, Signature) with well-defined composition rules per `FieldCategory`.
- The separation of parser generation (`parser_expr`) from printer generation (`print_expr`) sharing the same `FieldCategory` dispatch ensures that parse and print are structurally dual -- a key property for roundtrip correctness.
