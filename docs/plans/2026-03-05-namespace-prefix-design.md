# Namespace Prefixes for Dialect Composition

**Date:** 2026-03-05
**Status:** Approved

## Problem

When composing dialects via `#[wraps]`, all inner statements are parsed/printed with their bare names (e.g., `add`, `sub`). There is no way to add a namespace prefix like `arith.add` to disambiguate dialects or reflect hierarchical structure.

## Design

### User-Facing API

`#[chumsky(format = "...")]` on a `#[wraps]` variant specifies a namespace prefix (single identifier). On non-wraps variants, `format` retains its existing meaning as a full format string.

```rust
#[derive(HasParser, PrettyPrint)]
enum MyLang {
    #[wraps]
    #[chumsky(format = "arith")]
    Arith(Arith),

    #[chumsky(format = "ret {0}")]
    Ret(SSAValue),
}
```

- Parsing: `arith.add %x, %y -> i32` instead of `add %x, %y -> i32`
- Printing: `arith.add %x, %y -> i32` instead of `add %x, %y -> i32`

Multi-level nesting is compositional — each `#[wraps]` layer adds one prefix segment:

```rust
#[derive(HasParser, PrettyPrint)]
enum TopLevel {
    #[wraps]
    #[chumsky(format = "lang")]
    Lang(MyLang),
}
// Parses/prints: lang.arith.add %x, %y -> i32
```

### Validation

- `format` on a `#[wraps]` variant must be a single identifier (no dots, no field interpolations `{...}`).
- Compile error if violated: `"format on a #[wraps] variant must be a single identifier (namespace prefix)"`.

### Parser Codegen

In `build_wrapper_parser` (`codegen/parser/generate.rs`), when a `#[wraps]` variant has a `format` attribute:

1. Extract the namespace identifier from the format string.
2. Prepend a prefix parser before the delegated inner parser:

```rust
just(Token::Identifier("arith"))
    .then_ignore(just(Token::Dot))
    .ignore_then(
        WrappedType::recursive_parser::<...>(language.clone())
    )
    .map(|inner| AstName::Variant(inner))
```

No changes to `HasDialectParser` trait signatures.

### PrettyPrint Codegen

In the wrapper branch of `generate_pretty_print` (`codegen/pretty_print/statement.rs`), prepend namespace text before delegating:

```rust
doc.text("arith.") + PrettyPrint::pretty_print(inner, doc)
```

### Files Changed

| File | Change |
|---|---|
| `codegen/parser/generate.rs` | `build_wrapper_parser`: check for format, prepend prefix parser |
| `codegen/pretty_print/statement.rs` | wrapper branch: check for format, prepend prefix text |
| `codegen/helpers.rs` | Helper to extract namespace from format on wraps variants |
| `validation.rs` | Validate format on wraps variants is a single identifier |

### Testing

- Snapshot tests for parser codegen with namespace prefix
- Snapshot tests for pretty-print codegen with namespace prefix
- Integration test with multi-level nesting (compositional prefixes)
- Compile-fail test: `format = "arith.add"` on wraps variant (dots not allowed)
- Compile-fail test: `format = "add {0}"` on wraps variant (interpolation not allowed)
