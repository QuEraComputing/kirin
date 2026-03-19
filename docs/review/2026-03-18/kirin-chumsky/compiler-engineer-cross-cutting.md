# Compiler Engineer — Cross-Cutting Review: kirin-chumsky

## Build Graph

**Dependencies:** `chumsky`, `kirin-ir`, `kirin-lexer`, `kirin-prettyless`, `num-traits`, `rustc-hash`, `strsim`, optional `kirin-derive-chumsky`.

- **Hard dependency on `kirin-prettyless` is the most significant coupling concern.** The parser crate re-exports `PrettyPrint` from `kirin-prettyless` (`pub use kirin_prettyless::PrettyPrint`). This means any crate that depends on `kirin-chumsky` transitively depends on `kirin-prettyless` and its dependencies (`prettyless`, `bon`, optional `bat`, optional `serde`). A user who only needs parsing is forced to compile the printer. Consider making `kirin-prettyless` an optional dependency gated behind a `pretty` feature, with the re-export conditional on that feature.

- **`chumsky` is a heavy parser combinator library.** It brings in substantial compile-time cost due to deeply nested generic types. This is inherent to the parser combinator approach and not avoidable without switching parsing strategies. The `chumsky 0.10` version uses the `'t` lifetime everywhere, which is well-contained.

- **`strsim` dependency is used only for typo suggestions** in stage name resolution (`FunctionParseError`). This is a small crate with minimal impact. Good.

- **`num-traits` is used for numeric parsing builtins.** Reasonable.

- **Glob re-exports in `lib.rs`** (`pub use ast::*`, `pub use function_text::*`, `pub use parsers::*`, `pub use traits::*`) flatten four modules into the crate root namespace. This creates a large public API surface that couples downstream crates to internal module structure. Any addition to these modules becomes a public API change. The `prelude` module is more curated but still imports `pub use crate::parsers::*`.

## Scalability

- **`ParseDispatch` is monomorphic** -- each stage enum variant dispatches to a concrete dialect parser at compile time. This scales linearly with the number of dialects per stage (one match arm per dialect). With 50 dialects, the generated `ParseDispatch` match has 50 arms, which is fine for runtime but generates 50 monomorphized parser instantiations at compile time.

- **`HasDialectParser::Output<TypeOutput, LanguageOutput>` is a GAT with 2 type parameters.** Each dialect's parser produces `Output<T, L>` which must be `Clone + PartialEq + 't`. When composing N dialects into a language enum, the composite parser combines N parsers via `choice()`. Chumsky's `choice()` is a nested tuple of parsers, so with 50 dialects the parser type is a 50-element nested tuple. This stresses the type checker and produces enormous error messages on type mismatch.

- **`ParseEmit<L>` has three implementation paths**, which is good for flexibility but means the compiler must resolve which path applies. The blanket impl from `SimpleParseEmit` and the derive-generated impl from `HasParserEmitIR<'t>` are separate trait impls, so there is no ambiguity issue. Good design.

- **`first_pass_concrete` and `second_pass_concrete` are called per-dialect per function during pipeline parsing.** The two-pass approach (first pass: parse functions to AST, second pass: emit IR) means each function body is traversed twice. With many functions, this is O(functions * passes * statements). The per-function allocation pattern (building `EmitContext` per function) is reasonable.

## Error Quality

- **`FunctionParseError` provides structured error information** with `FunctionParseErrorKind` variants. The stage name typo detection via `strsim` is a nice touch.

- **`ParseError` is minimal** (`message: String, span: SimpleSpan`). It wraps chumsky's `Rich` errors, which provide good location information. The `Display` impl shows `error at start..end: message`.

- **`ParseStatementText` error on missing stage** produces a `ParseError` with span `0..0`, which is semantically correct (the error is not located in the input text) but looks odd in error output: "error at 0..0: stage Stage(Id(5)) not found in pipeline". Including the stage ID is helpful.

- **`parse_derive_input` in the derive crate handles missing `#[kirin(type)]` gracefully** by attempting to parse without it, then checking if SSA/Result fields require it. The error message is clear: "`#[kirin(type = ...)]` is required when using SSAValue, ResultValue, Block, or Region fields."

- **Chumsky's error recovery** means parse failures produce multiple error messages. Users may see cascading errors from a single typo. This is inherent to the parser combinator approach and not easily fixable.

## Compilation Time

- **Chumsky parser types are deeply nested generics.** The `BoxedParser<'t, I, O>` type alias hides the complexity, but the underlying `Boxed<'t, 't, I, O, ParserError<'t>>` is a boxed trait object. Boxing is essential to keep compile times reasonable -- without it, the parser type would be a deeply nested generic monster.

- **`HasDialectParser` has 3 type parameters on `Output` plus 3 more on the method generics** (`I`, `TypeOutput`, `LanguageOutput`). Each dialect's `namespaced_parser` method monomorphizes over `I` and the two output types. With the boxed parser return type, this is bounded, but the method bodies still generate code for each monomorphization.

- **`ParseStatementText` has a default type parameter `Ctx = ()`.** The blanket `ParseStatementTextExt` adds a layer of indirection but should be zero-cost after monomorphization. The three impls (for `StageInfo<L>`, `BuilderStageInfo<L>`, and `Pipeline<S>`) are straightforward.

- **The `prelude` re-exports `chumsky::prelude::*`**, pulling chumsky's types into every file that uses the prelude. This is convenient but means changes to chumsky's prelude affect downstream compilation. Since chumsky is a workspace dependency, version changes are controlled.

## Summary

- **P1** [confirmed] Hard dependency on `kirin-prettyless` forces all parser users to compile the printer; should be feature-gated — `crates/kirin-chumsky/Cargo.toml:10`
- **P2** [likely] Glob re-exports (`pub use ast::*`, etc.) in `lib.rs` flatten internal modules into the public API, creating unnecessary coupling — `crates/kirin-chumsky/src/lib.rs:60-63`
- **P3** [informational] Composite parser types with many dialects produce nested tuple types that stress the type checker and generate poor error messages — inherent to chumsky combinator approach
- **P3** [informational] Two-pass pipeline parsing traverses function bodies twice; acceptable for correctness but doubles parse-phase cost
