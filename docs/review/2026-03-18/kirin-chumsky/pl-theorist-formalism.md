# PL Theorist — Formalism Review: kirin-chumsky

## Abstraction Composability

### Three-path ParseEmit design

The `ParseEmit<L>` trait (`traits/parse_emit.rs:17-23`) provides three implementation paths:

1. **Derive-generated** — `#[derive(HasParser)]` auto-generates `ParseEmit`.
2. **Marker-based** — `SimpleParseEmit` marker trait provides a blanket impl via `for<'t>` quantification.
3. **Manual** — direct implementation for full control.

This is a classic three-tier API design: automatic (derive), semi-automatic (marker), and manual. The blanket impl for `SimpleParseEmit` (`parse_emit.rs:33-51`) uses higher-rank trait bounds (`for<'t> L: HasParser<'t>` and `for<'t> <L as HasParser<'t>>::Output: EmitIR<L>`) to bridge the lifetime-parameterized parsing with the lifetime-free emission.

The three paths compose independently — implementing `SimpleParseEmit` does not interfere with `HasDialectParser`, and the derive-generated path produces a dedicated `ParseEmit` impl. There is no risk of overlapping impls because the marker trait prevents the blanket from firing for derive-annotated types.

### HasParser vs HasDialectParser split

`HasParser<'t>` (`traits/has_parser.rs:22-29`) is for non-recursive types (type lattices, compile-time values). `HasDialectParser<'t>` (`traits/has_parser.rs:42-86`) is for dialect types that need recursive parsing (blocks, regions). The split encodes a fundamental distinction:

- `HasParser<'t>` returns a simple `BoxedParser<'t, I, Output>` — no recursion handle.
- `HasDialectParser<'t>` takes a `RecursiveParser` handle for parsing nested constructs, and has a GAT `Output<TypeOutput, LanguageOutput>` parameterized by the type and language AST types.

This separation prevents circular parser construction: simple types parse independently, while dialect types receive the recursive handle explicitly. The GAT on `HasDialectParser` avoids the need for a fixed AST type — different compositions produce different `Output` types.

### EmitIR: AST-to-IR phase separation

`EmitIR<L>` (`traits/emit_ir.rs:121-124`) defines the AST-to-IR lowering. The trait is parameterized by the target dialect `L` and uses an `EmitContext` that carries mutable builder state, SSA name maps, and block name maps.

The `DirectlyParsable` marker trait (`traits/emit_ir.rs:131`) provides identity emission for types that parse directly into IR values (no AST intermediate). This is the correct optimization for type lattices and compile-time values.

The `EmitIR` impls for `Vec<T>` and `Option<T>` (`emit_ir.rs:149-171`) provide structural recursion, which is standard for a traversal algebra.

### HasDialectEmitIR: witness trait for GAT projection

`HasDialectEmitIR<'tokens, Language, LanguageOutput>` (`traits/has_dialect_emit_ir.rs:52-73`) exists because:

1. Dialect-specific bounds (e.g., `T: HasParser + EmitIR`) cannot be expressed on `HasDialectParser`'s language-agnostic methods.
2. GAT projections under `for<'t>` cause E0275 with self-referential AST types.

The trait takes a `LanguageOutput` type parameter and an `emit_language_output` callback for recursive emission. This replaces trait-solver recursion with explicit control-flow recursion — a defunctionalization technique similar to the `L`-on-method pattern in the interpreter.

The callback pattern `&EmitLanguageOutput: for<'ctx> Fn(&LanguageOutput, &mut EmitContext<'ctx, Language>) -> Result<Statement, EmitError>` is a rank-2 type that ensures the callback works with any `EmitContext` lifetime. This is the CPS encoding of recursive emission.

### ParseStatementText with default context parameter

`ParseStatementText<L, Ctx = ()>` (`traits/parse_text.rs:31-37`) uses a default type parameter to unify two calling conventions:

- `StageInfo<L>: ParseStatementText<L, ()>` — no extra context needed.
- `Pipeline<S>: ParseStatementText<L, CompileStage>` — stage ID required.

The `ParseStatementTextExt<L>` blanket (`parse_text.rs:43-55`) erases the `()` context for ergonomic calls. This is a standard default-parameter technique for trait overloading.

## Literature Alignment

### Parser combinator foundations

The crate builds on Chumsky, which implements parsing with context-free grammars via monadic parser combinators (Hutton & Meijer, 1998). The recursive parser handle (`RecursiveParser`) corresponds to the `fix` combinator for recursive grammars.

The namespace filtering (`HasDialectParser::namespaced_parser`) adds a prefix-based dispatch layer on top of the parser combinator algebra. This corresponds to prioritized choice with a namespace guard — each dialect's parser only fires when the keyword matches its namespace prefix.

### Two-phase parsing: AST then IR emission

The parse-then-emit architecture (`parse_ast` -> `EmitIR`) follows the standard compiler pipeline separation between syntactic analysis and semantic lowering. The `EmitContext` with mutable name maps corresponds to a symbol table environment threaded through the semantic analysis pass.

The `set_relaxed_dominance` feature (`emit_ir.rs:79-85`) enables forward references, which is necessary for cyclic graph bodies where definitions may reference values not yet defined. This corresponds to "relaxed dominance" in MLIR's graph regions where use-def ordering is not enforced.

### Witness methods for GAT bounds

`HasDialectParser` uses `clone_output` and `eq_output` witness methods (mentioned in AGENTS.md) to solve GAT projection E0275 for `Clone`/`PartialEq` bounds. This is the **dictionary-passing** technique from Haskell, manually applied: instead of requiring `Output<T, L>: Clone`, the trait provides a `clone_output` method that the impl fills in with the concrete clone logic. This avoids asking the trait solver to project through GATs.

## Semantic Ambiguity

### `ParseError` vs `EmitError` vs `FunctionParseError`

The crate defines three error types:
- `ParseError` (`has_parser.rs:89-105`) — syntax errors with spans.
- `EmitError` (`emit_ir.rs:6-13`) — semantic errors during AST-to-IR lowering.
- `FunctionParseError` — pipeline-level parsing errors.

The `parse_and_emit` method on `ParseEmit` returns `Result<Statement, Vec<ParseError>>`, conflating emit errors with parse errors by wrapping `EmitError` in a `ParseError` with a zero span (`parse_emit.rs:44-49`). This loses the distinction between "the text didn't parse" and "the text parsed but the IR couldn't be built". Downstream consumers cannot distinguish these cases.

### `EmitContext::resolve_ssa` forward reference semantics

In relaxed dominance mode, `resolve_ssa` (`emit_ir.rs:63-73`) creates `Unresolved(Result(0))` placeholders for forward references. The `Result(0)` is a fixed index, meaning all forward references get the same resolution info variant. This is correct if the builder resolves them later by matching on name, but the fixed index `0` could be confusing — it does not correspond to the actual result index. The `ResolutionInfo::Result(usize)` variant's `usize` field has a different meaning for forward refs vs. normal results.

### `BoxedParser` type-erasure overhead

All parsers are boxed (`BoxedParser`), which introduces dynamic dispatch and allocation. This is a pragmatic choice for composability (different parsers can have different concrete types), but it means the parser combinator algebra loses the zero-cost abstraction property of direct combinator composition. For a compiler framework where parsing is not typically a bottleneck, this is acceptable.

## Alternative Formalisms Considered

### 1. Parse-emit separation vs. direct semantic actions

**Current**: Two-phase — parse to AST (`HasParser`/`HasDialectParser`), then lower to IR (`EmitIR`).
**Alternative A**: Direct semantic actions — parser combinators produce IR directly (as in YACC-style semantic actions).
**Alternative B**: Attribute grammars — synthesized/inherited attributes flow through the parse tree.

| Metric | Two-phase (current) | Direct actions | Attribute grammars |
|--------|---------------------|----------------|-------------------|
| Separation of concerns | Clean (syntax vs. semantics) | Tangled | Medium |
| Error reporting | Better (AST available) | Harder | Medium |
| Composability | High (AST reusable) | Low | Medium |
| Performance | Two passes | One pass | One pass |
| Implementation complexity | Medium | Low | High |

Two-phase is the standard choice for MLIR-style frameworks where the AST serves as an intermediate representation for error reporting and diagnostics.

### 2. Recursive parser handle: explicit vs. implicit

**Current**: Explicit `RecursiveParser` handle passed to `HasDialectParser::namespaced_parser`.
**Alternative A**: Implicit via trait method (parser returns a `Parser` that can call `Self::parser()` recursively).
**Alternative B**: Lazy evaluation (thunked parsers).

| Metric | Explicit handle (current) | Implicit recursion | Lazy |
|--------|--------------------------|--------------------|----|
| Cycle safety | Guaranteed (handle is the fixpoint) | Risk of infinite loop | Safe (thunk breaks cycle) |
| Composability | Must thread handle | Automatic | Automatic |
| Performance | Good (single fixpoint) | Potential multiple fixpoints | Thunk overhead |

The explicit handle is the principled approach — it corresponds to the `fix` operator in the recursion theory of parser combinators.

### 3. Namespace dispatch: keyword prefix vs. grammar-level choice

**Current**: Namespace filtering via string prefix matching in `namespaced_parser`.
**Alternative A**: Grammar-level prioritized choice (`dialect_a_parser.or(dialect_b_parser)`).
**Alternative B**: Token-level dispatch (first token determines which dialect to parse).

| Metric | Namespace prefix (current) | Grammar choice | Token dispatch |
|--------|---------------------------|----------------|---------------|
| Disambiguation | Clear (namespace is authoritative) | Requires priority ordering | Requires disjoint first-sets |
| Error messages | Good ("expected namespace X") | Poor (all alternatives fail) | Good |
| Composability | Additive (new namespaces don't conflict) | Order-dependent | Requires first-set analysis |

Namespace prefix dispatch is the correct choice for a dialect-oriented framework where each dialect "owns" a namespace prefix.

## Summary

- [P2] [confirmed] `ParseEmit::parse_and_emit` conflates `EmitError` with `ParseError`, losing error origin — `traits/parse_emit.rs:44-49`
- [P3] [confirmed] `EmitContext::resolve_ssa` forward-reference uses fixed `Result(0)` index with overloaded semantics — `traits/emit_ir.rs:114`
- [P3] [confirmed] Three error types (`ParseError`, `EmitError`, `FunctionParseError`) without a unified hierarchy — `has_parser.rs:89`, `emit_ir.rs:6`, `function_text/error.rs`
- [P3] [informational] Witness method technique for GAT projection is well-motivated — `has_parser.rs:42-86`
- [P3] [informational] Three-path ParseEmit design provides good flexibility/ergonomics tradeoff — `parse_emit.rs:17-51`
