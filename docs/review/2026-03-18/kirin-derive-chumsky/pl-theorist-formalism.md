# PL Theorist — Formalism Review: kirin-derive-chumsky

## Abstraction Composability

### Format string DSL

The format string DSL (`format.rs:22-154`) defines a mini-language for specifying parser syntax:

```
format_string := (escaped_brace | keyword | field | literal)*
keyword       := '{' '.' IDENT '}'
field         := '{' (IDENT | INT) (':' ('name' | 'type'))? '}'
escaped_brace := '{{' | '}}'
literal       := TOKEN+
```

This is a **context-free grammar** embedded as a string attribute. The `FormatElement` sum type represents the parsed AST of the format string:
- `Token(Vec<Token>)` — literal tokens
- `Field(&str, FormatOption)` — field interpolation with optional name/type projection
- `Keyword(&str)` — namespace-prefixed keyword

The format DSL composes cleanly with the field classification from the toolkit: each `{field}` reference in the format string must correspond to a field in the struct/variant, and the code generator maps format elements to parser combinators.

The DSL is self-hosting in that it uses the Kirin lexer (`kirin_lexer::Token`) and Chumsky parser combinators to parse itself. This bootstrapping is elegant but means the format string syntax is constrained to the token vocabulary of the Kirin lexer.

### Three code generation phases

The `#[derive(HasParser)]` macro generates three components:

1. **AST type** (`GenerateAST`) — a parallel AST type with the same structure as the dialect type but using parser-friendly types (e.g., `String` instead of `SSAValue`).
2. **HasDialectParser impl** (`GenerateHasDialectParser`) — parser combinator chains derived from format strings.
3. **EmitIR impl** (`GenerateEmitIR`) — AST-to-IR lowering logic.

These three phases correspond to the standard compiler pipeline (syntax definition, parsing, lowering) but applied at the metaprogramming level. The phases are independent: the AST type definition does not depend on the parser logic, and the emit logic depends on the AST type but not the parser implementation.

The additional fourth component — `HasDialectEmitIR` (`generate.rs:37`) — provides the witness trait for recursive emission through `#[wraps]` variants.

### Parser chain construction

`generate_enum_parser_body` (`generate.rs:77-107`) composes variant parsers using `.or()`:

```rust
variant_parsers.into_iter()
    .reduce(|acc, parser| quote! { #acc.or(#parser) })
```

This is the standard alternation combinator for CFGs. The ordering is left-to-right (first variant has priority), which means the parser is **ordered** — if two variants could parse the same prefix, the first one wins. This is consistent with PEG semantics rather than CFG semantics.

For `#[wraps]` variants, the parser delegates to the inner type's `HasDialectParser::namespaced_parser`, threading the namespace filter. This provides compositional namespace scoping: each wrapper adds its format string as a namespace prefix, and the inner parser filters by that prefix.

### PrettyPrint generation

The `#[derive(PrettyPrint)]` macro (`lib.rs:97-108`) uses a separate `PrettyPrintLayout` that shares `ChumskyStatementAttrs` and `ChumskyFieldAttrs` with the parser derive. This reuse ensures that the same format string drives both parsing and printing, maintaining the **roundtrip invariant** (`parse(sprint(ir)) == ir`).

The pretty-print generation reverses the parser chain: where the parser reads tokens and constructs fields, the printer reads fields and emits tokens. This duality is a classical property of bidirectional transformations (lenses, prisms) in PL theory.

## Literature Alignment

### Syntax-directed translation

The derive macro implements a form of **syntax-directed translation** (Aho et al., "Compilers: Principles, Techniques, and Tools"): the format string defines both the syntax and the semantic actions. The `FormatElement` sequence determines the parser combinator chain, and the field references determine how parsed values flow into the AST constructor.

This is more restricted than a general attribute grammar (no inherited attributes, no semantic predicates) but sufficient for MLIR-style operation syntax where each operation has a fixed format.

### Format string as concrete syntax specification

The format string DSL is analogous to MLIR's **ODS (Operation Definition Specification)** format directive:

```
// MLIR ODS
let assemblyFormat = "$result `=` `add` $lhs `,` $rhs `:` type($result)";

// Kirin format string
#[chumsky(format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]
```

The key differences:
- Kirin uses `{.keyword}` for namespace-prefixed keywords; MLIR uses backtick-quoted literals.
- Kirin uses `{field:name}` / `{field:type}` projections; MLIR uses `type($field)` / `$field`.
- Kirin's format string is parsed at proc-macro time into `FormatElement`s; MLIR's is parsed by tablegen.

The alignment with MLIR's design philosophy is strong — both derive parser and printer from a single declarative format specification.

### Bidirectional transformations

The parser/printer duality achieved by `#[derive(HasParser)]` and `#[derive(PrettyPrint)]` sharing the same format string implements a restricted form of **bidirectional transformation** (Bohannon et al., "Boomerang: Resourceful Lenses for String Data", 2008). The format string is the shared "lens" that defines both directions. The restriction is that the transformation is not truly invertible in general — the parser may accept more inputs than the printer produces (e.g., whitespace flexibility).

## Semantic Ambiguity

### Format string field reference resolution

Field references in format strings (`{name}` or `{0}`) are resolved against the struct/variant fields. The resolution is done by name for named fields and by index for positional fields. The `FormatOption::Name` and `FormatOption::Type` projections only apply to certain field types (`SSAValue`, `ResultValue`). For other field types, `:name` and `:type` may silently produce incorrect behavior if the field type doesn't support the projection. The validation pass (`validation.rs`) should catch these, but the format string grammar itself permits any field with any projection.

### Namespace prefix composition

When a `#[wraps]` variant has a `#[chumsky(format = "arith")]` attribute, the format string becomes a namespace prefix. The wrapper's parser prepends this prefix to the inner parser's namespace array. However, the composition rule for nested wrappers is additive — `A::B::C` would accumulate namespaces `["A", "B", "C"]`. The inner parser sees the full namespace chain. This could cause ambiguity if different nesting paths produce the same namespace array.

### `.or()` ordering semantics

The enum variant parser uses left-to-right `.or()` composition (`generate.rs:102-106`). This means variant ordering in the enum definition determines parsing priority. If two variants have overlapping first-token sets, the first variant wins silently. There is no ambiguity detection or warning. This is the standard PEG semantics, but users familiar with CFG-based parsers might expect longest-match or ambiguity errors.

## Alternative Formalisms Considered

### 1. Format string DSL vs. combinator API vs. grammar file

**Current**: Format strings in attributes (`#[chumsky(format = "...")]`).
**Alternative A**: Combinator API — users write parser combinators directly.
**Alternative B**: External grammar file (like ANTLR `.g4` or MLIR tablegen).

| Metric | Format string (current) | Combinator API | Grammar file |
|--------|------------------------|----------------|-------------|
| Conciseness | High (one-liner syntax) | Low (verbose) | Medium |
| Learning curve | Low (familiar syntax) | Medium (combinator API) | Low (grammar notation) |
| Flexibility | Limited (fixed patterns) | Full | Medium |
| Roundtrip guarantee | Structural (shared format) | None (separate printer) | Possible |
| Tooling | None | Editor support | Grammar IDE |

Format strings are the right choice for a framework that prioritizes conciseness and roundtrip guarantees. The limited flexibility is acceptable because MLIR-style operations have regular syntax.

### 2. AST generation: parallel type vs. GAT vs. untyped

**Current**: Parallel AST type (`FooAST`) generated alongside the dialect type.
**Alternative A**: GAT on `HasDialectParser` where `Output<T, L>` is the AST type family.
**Alternative B**: Untyped AST (parse to `HashMap<String, Value>`).

| Metric | Parallel type (current) | GAT | Untyped |
|--------|------------------------|-----|---------|
| Type safety | Full | Full | None |
| Generated code volume | High (full type + impls) | Low (reuse existing types) | Low |
| Composition | Clean (separate type) | Complex (GAT projections) | Simple |
| Debug experience | Good (concrete types) | Poor (projected types) | Poor |

The parallel type approach generates more code but provides the best debugging experience and avoids GAT projection issues. The GAT approach was tried but ran into E0275, justifying the current design.

### 3. Roundtrip via format string vs. quotient type vs. normalization

**Current**: Shared format string drives both parser and printer.
**Alternative A**: Quotient types — define an equivalence class on ASTs and normalize before comparison.
**Alternative B**: Canonical forms — always normalize to a specific representation.

| Metric | Shared format (current) | Quotient types | Canonical forms |
|--------|------------------------|----------------|----------------|
| Roundtrip guarantee | By construction | By proof | By normalization |
| Implementation cost | Low (derive macro) | High | Medium |
| Flexibility | Low (one syntax per op) | High | Medium |

Shared format strings provide the simplest roundtrip guarantee. The limitation is that each operation has exactly one textual representation, which is usually acceptable for IR text formats.

## Summary

- [P2] [likely] `.or()` ordering in enum variant parsers creates implicit priority without ambiguity detection — `codegen/parser/generate.rs:102-106`
- [P3] [confirmed] Format string DSL is self-bootstrapping via Kirin lexer, constraining its token vocabulary — `format.rs:130-135`
- [P3] [confirmed] Namespace prefix composition is additive but composition semantics for deeply nested wrappers could be clearer — `codegen/parser/generate.rs:194-208`
- [P3] [informational] Format string drives both parsing and printing, implementing a restricted bidirectional transformation — `lib.rs:71-108`
- [P3] [informational] Three-phase code generation (AST + parser + emitter) follows standard compiler pipeline at the meta-level — `codegen/parser/generate.rs:26-45`
