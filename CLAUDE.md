# CLAUDE.md

Guidance for Claude Code (claude.ai/code) when working with this repository.

## Build and Test

```bash
cargo build --workspace          # Build all crates
cargo test --workspace           # Run all tests
cargo test -p kirin-chumsky      # Test a single crate
cargo test -p kirin-chumsky-derive test_parse_add  # Run a single test
cargo fmt --all                  # Format code
cargo insta review               # Review snapshot test changes
```

Rust edition 2024. No `rust-toolchain.toml`; uses the default toolchain.

## Design Documentation

See [`design/`](./design/) for architecture overview:
- [`design/ir.md`](./design/ir.md) — Dialect composability and type system
- [`design/text-format.md`](./design/text-format.md) — Parser and pretty printer

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>): <description>`

Examples: `feat(chumsky): add region parser`, `fix(derive): handle empty enum variants`

## Crate Map

**Core:**
- `kirin-ir` — IR types, `Dialect` trait
- `kirin-lexer` — Logos tokenizer

**Parser/Printer:**
- `kirin-chumsky` — Parser traits (`HasParser`, `HasRecursiveParser`, `EmitIR`)
- `kirin-prettyless` — Pretty printer (`PrettyPrint`)
- `kirin-chumsky-derive` — `#[derive(HasParser, PrettyPrint)]`
- `kirin-chumsky-format` — Code generation (internal)

**Dialects:**
- `kirin-cf`, `kirin-scf`, `kirin-constant`

**Derive Infrastructure:**
- `kirin-derive-core` — Shared derive utilities
- `kirin-derive`, `kirin-derive-dialect` — `#[derive(Dialect)]`

## Key Files

- Parser traits: `kirin-chumsky/src/traits.rs`
- AST types: `kirin-chumsky/src/ast.rs`
- Code generators: `kirin-chumsky-format/src/generate/`
- Snapshot tests: `kirin-chumsky-format/src/snapshots/`
- Integration tests: `kirin-chumsky-derive/tests/`

Note: Crates suffixed `-old` are prior iterations, not workspace members.
