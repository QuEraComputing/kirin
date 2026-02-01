# Text Format

Parser and pretty printer integration for Kirin IR.

## Roundtrip Pipeline

```
Source Text → Parser → AST → EmitIR → IR → PrettyPrint → Source Text
```

## Usage

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(type_lattice = MyType)]
#[chumsky(crate = kirin_chumsky)]
pub enum MyDialect {
    #[chumsky(format = "{res:name} = add {lhs} {rhs}")]
    Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
}
```

## Format String Syntax

- **`{field}`** — Parse/print with default format
- **`{field:name}`** — Name only (for SSA/Result)
- **`{field:type}`** — Type only (for SSA/Result)
- **`{0}`, `{1}`** — Positional fields

## Architecture

```mermaid
flowchart LR
    HP["#[derive(HasParser)]"] --> AST["Generated AST"]
    HP --> Parser["HasRecursiveParser impl"]
    HP --> Emit["EmitIR impl"]
    PP["#[derive(PrettyPrint)]"] --> Print["PrettyPrint impl"]
```

## Design Highlights

- **Two-phase parsing** — Parser produces AST, then `EmitIR` converts to IR. This separation enables AST inspection and better error messages.

- **Generic AST over Language** — `MyDialectAST<'t, 's, Language>` allows the same AST to be used when parsing composed dialects.

- **AST types generic over TypeOutput** — `SSAValue<'src, TypeOutput>` avoids requiring `TypeLattice: HasParser` at struct definition time.

- **Method-level trait bounds** — The `TypeLattice: HasParser` bound is on `recursive_parser()` method, not the `HasRecursiveParser` trait, to avoid circular resolution.

- **Roundtrip fidelity** — SSA names from source (e.g., `%x`) are preserved in IR for exact roundtrip.

## Key Code Locations

- **Parser traits** — `kirin-chumsky/src/traits.rs`
- **AST types** — `kirin-chumsky/src/ast.rs`
- **Parser combinators** — `kirin-chumsky/src/parsers.rs`
- **Derive macros** — `kirin-chumsky-format/src/generate/`
- **Tests** — `kirin-chumsky-derive/tests/`
