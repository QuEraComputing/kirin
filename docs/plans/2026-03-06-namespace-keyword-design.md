# Namespace Prefixes for Dialect Composition (v2)

**Date:** 2026-03-06
**Status:** Approved
**Supersedes:** 2026-03-05-namespace-prefix-design.md

## Problem

When composing dialects via `#[wraps]`, all inner statements are parsed/printed with their bare keyword names (e.g., `add`, `sub`). There is no way to add a namespace prefix like `arith.add` to disambiguate or reflect hierarchy, as in MLIR's `arith.add` / `scf.if` convention.

The v1 design prepended the namespace before the *entire* inner statement (`arith.%res = add ...`). The correct behavior is keyword-level namespacing (`%res = arith.add ...`), which requires the inner parser/printer to know about the namespace.

## Design

### `{.keyword}` Format Syntax

A new format element marks the operation keyword — the identifier that gets namespace-prefixed:

```rust
// Assignment pattern:
#[chumsky(format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]

// No-assignment pattern:
#[chumsky(format = "{.return} {0}")]

// Body-only (no keyword):
#[chumsky(format = "{body}")]
```

Inside the format mini-language, `{.X}` is parsed as `FormatElement::Keyword("X")`. The dot-prefix is unambiguous with existing syntax (`{field}`, `{field:name}`, `{field:type}`).

### HasDialectParser Trait Change

```rust
pub trait HasDialectParser<'tokens, 'src: 'tokens>: Sized {
    type Output<TypeOutput, LanguageOutput>: Clone + PartialEq
    where
        TypeOutput: Clone + PartialEq + 'tokens,
        LanguageOutput: Clone + PartialEq + 'tokens;

    /// Backward-compatible entry point.
    fn recursive_parser<I, TypeOutput, LanguageOutput>(
        language: RecursiveParser<...>,
    ) -> BoxedParser<...>
    where ...
    {
        Self::namespaced_parser::<I, TypeOutput, LanguageOutput>(language, &[])
    }

    /// Parser with namespace prefix applied to `{.keyword}` tokens.
    fn namespaced_parser<I, TypeOutput, LanguageOutput>(
        language: RecursiveParser<...>,
        namespace: &[&'static str],
    ) -> BoxedParser<...>
    where ...;
}
```

- `recursive_parser` gets a default impl that calls `namespaced_parser(&[])`.
- Derive generates `namespaced_parser`. Manual impls implement `namespaced_parser`.

### PrettyPrint Trait Change

```rust
pub trait PrettyPrint {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(
        &self, doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where L::Type: core::fmt::Display
    {
        self.namespaced_pretty_print(doc, &[])
    }

    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self, doc: &'a Document<'a, L>, namespace: &[&str],
    ) -> ArenaDoc<'a>
    where L::Type: core::fmt::Display;
}
```

Same pattern: `pretty_print` delegates to `namespaced_pretty_print` with `&[]`.

### Codegen for `{.keyword}`

**Parser codegen** — `{.add}` generates a runtime-branching token parser:

```rust
// Generated inside namespaced_parser:
{
    let keyword_parser = if namespace.is_empty() {
        just(Token::Identifier("add")).boxed()
    } else {
        let mut p = just(Token::Identifier(namespace[0]));
        for &ns in &namespace[1..] {
            p = p.then_ignore(just(Token::Dot)).then_ignore(just(Token::Identifier(ns)));
        }
        p.then_ignore(just(Token::Dot)).then_ignore(just(Token::Identifier("add"))).boxed()
    };
    keyword_parser
}
```

**Printer codegen** — `{.add}` generates:

```rust
// Generated inside namespaced_pretty_print:
{
    let keyword_text = if namespace.is_empty() {
        "add".to_string()
    } else {
        let mut s = namespace.join(".");
        s.push('.');
        s.push_str("add");
        s
    };
    doc.text(keyword_text)
}
```

### Wrapper Codegen

For a `#[wraps]` variant with `#[chumsky(format = "arith")]`:

**Parser:**
```rust
{
    let mut ns = namespace.to_vec();
    ns.push("arith");
    WrappedType::namespaced_parser::<I, T, L>(language.clone(), &ns)
        .map(|inner| -> ReturnType { Constructor(inner) })
}
```

**PrettyPrint:**
```rust
{
    let mut ns = namespace.to_vec();
    ns.push("arith");
    inner.namespaced_pretty_print(doc, &ns)
}
```

### User-Facing API

```rust
#[derive(HasParser, PrettyPrint)]
#[wraps]
enum MyLang {
    #[chumsky(format = "arith")]
    Arith(Arith),

    #[chumsky(format = "{.ret} {0}")]
    Ret(SSAValue),
}

// Parses/prints: %res = arith.add %a, %b -> i64
// Parses/prints: ret %v
```

Multi-level nesting is compositional:

```rust
enum TopLevel {
    #[chumsky(format = "lang")]
    Lang(MyLang),
}
// Parses/prints: %res = lang.arith.add %a, %b -> i64
```

### Migration

All ~30 existing format strings must replace bare keyword identifiers with `{.keyword}`:

```
// Before:
#[chumsky(format = "{result:name} = add {lhs}, {rhs} -> {result:type}")]

// After:
#[chumsky(format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]
```

Body-only formats like `{body}` are unchanged.

### Prerequisite (Already Done)

`Token::Dot` was added to the lexer and `.` / `$` were removed from identifier character classes (commit a694d4f3e).

### Files Changed

| Area | Files |
|---|---|
| Format parser | `kirin-derive-chumsky/src/format.rs` — add `FormatElement::Keyword`, parse `{.X}` |
| Traits | `kirin-chumsky/src/traits/has_parser.rs` — add `namespaced_parser` |
| Traits | `kirin-prettyless/src/traits.rs` — add `namespaced_pretty_print` |
| Parser codegen | `kirin-derive-chumsky/src/codegen/parser/` — keyword-aware chain building |
| Printer codegen | `kirin-derive-chumsky/src/codegen/pretty_print/` — keyword-aware printing |
| Wrapper codegen | `kirin-derive-chumsky/src/codegen/parser/generate.rs` — namespace threading |
| Validation | `kirin-derive-chumsky/src/validation.rs` — validate `{.keyword}` usage |
| Migration | All crates with `#[chumsky(format = "...")]` — update to `{.keyword}` |
| Tests | Format parser tests, codegen snapshots, integration roundtrip, multi-level nesting |

### Testing

1. Format parser: `{.keyword}` parses as `FormatElement::Keyword`
2. Codegen snapshots: keyword-aware parser chain and pretty-print
3. Integration roundtrip: `%res = arith.add %a, %b -> i64` with CompositeLanguage
4. Multi-level nesting: `lang.arith.add` with two wrapper layers
5. No-namespace case: standalone dialect with `{.keyword}` works without prefix
6. Validation: error if format string has no `{.keyword}` but wraps variant has namespace
