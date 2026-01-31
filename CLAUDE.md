# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands

```bash
cargo build --workspace          # Build all crates
cargo test --workspace           # Run all tests
cargo test -p kirin-chumsky      # Test a single crate
cargo test -p kirin-chumsky-derive test_parse_add  # Run a single test
cargo fmt --all                  # Format code
cargo insta review               # Review snapshot test changes (kirin-chumsky-format)
```

Rust edition 2024. No `rust-toolchain.toml`; uses the default toolchain.

Snapshot tests use `insta`. After changing generated code, run `cargo insta review` to accept/reject updated snapshots in `crates/kirin-chumsky-format/src/snapshots/`.

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/) format: `<type>(<scope>): <description>`, e.g. `feat(chumsky): add region parser`, `fix(derive): handle empty enum variants`, `refactor(lexer): rename Token variants`.

## Architecture

Kirin is a compiler IR framework. The active work centers on derive macros that auto-generate chumsky parsers for dialect definitions. The codebase follows a three-layer architecture:

### Runtime Layer (`kirin-chumsky`)
Defines the trait system that all parsers implement:
- **`HasParser`** — non-recursive parsers (type lattices, simple constructs)
- **`HasRecursiveParser`** — recursive parsers for dialects containing blocks/regions; takes a language-level recursive parser handle
- **`WithAbstractSyntaxTree`** — maps IR types to AST types used during parsing
- **`LanguageParser`** — marker trait; any type implementing `Dialect + HasRecursiveParser` gets a blanket `HasParser` impl via `chumsky::recursive`

Also provides common AST nodes (`SSAValue`, `ResultValue`, `Block`, `Region`, `BlockHeader`, etc.) and parser combinators for them. All parsers operate on `kirin_lexer::Token` streams.

### Code Generation Layer (`kirin-chumsky-format`)
Proc-macro logic (but not the proc-macro entry points themselves):
- **Format string parser** (`format.rs`) — parses `#[chumsky(format = "...")]` strings into field interpolations and literal tokens. Supports `{field}`, `{field:name}`, `{field:type}`, positional `{0}`, and escaped `{{`/`}}`.
- **AST generator** (`generate/ast.rs`) — produces an AST enum/struct with lifetimes and a `Language` generic, plus the `WithAbstractSyntaxTree` impl.
- **Parser generator** (`generate/parser.rs`) — produces `HasRecursiveParser` impl by composing combinators from the format string.
- **Attribute parsing** (`attrs.rs`) — handles `#[chumsky(...)]` and `#[kirin(...)]` attributes via the `ChumskyLayout` trait (built on `kirin-derive-core`'s `Layout` system and `darling`).

### Macro Entry Points (`kirin-chumsky-derive`)
Thin proc-macro crate exposing three derives:
- `HasRecursiveParser` — parser only
- `WithAbstractSyntaxTree` — AST type only
- `DialectParser` — both combined

### Supporting Crates
- **`kirin-lexer`** — Logos-based tokenizer (`Token` enum with SSA values, blocks, symbols, identifiers, literals, punctuation)
- **`kirin-ir`** — Core IR types and the `Dialect` trait (with `HasArguments`, `HasResults`, `HasBlocks`, `HasRegions`, `HasSuccessors`, `IsTerminator`, `IsConstant`, `IsPure`)
- **`kirin-derive-core`** — Shared derive infrastructure: `Input<L>`, `Statement`, `Data` (struct/enum), field categorization (SSA, result, block, region, successor). Uses `darling` for attribute parsing.
- **`kirin-derive` / `kirin-derive-dialect`** — Base `Dialect` derive macros
- **`kirin-cf`, `kirin-scf`, `kirin-constant`** — Dialect crates (control flow, structured control flow, constants)

### Key Design Patterns
- **Format strings drive codegen**: each enum variant or struct gets a `#[chumsky(format = "...")]` attribute that defines its concrete syntax. The format parser and generators in `kirin-chumsky-format` turn this into parser combinator code.
- **Language composition**: dialects implement `HasRecursiveParser<L>` parameterized on a language `L`. A top-level language enum composes dialects, and the blanket `LanguageParser` impl wires up `chumsky::recursive`.
- **IR-to-AST mapping**: `WithAbstractSyntaxTree` bridges the gap between IR node types (e.g., `kirin_ir::SSAValue`) and parser output types (e.g., `ast::SSAValue<'tokens, 'src, L>`).
- Crates suffixed `-old` in the `crates/` directory are prior iterations; they are not workspace members.
