+++
rfc = "0001"
title = "Function parser interface"
status = "Implemented"
agents = ["codex", "claude opus"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-08T03:49:05.889848Z"
last_updated = "2026-02-08T06:50:01Z"
+++

# RFC 0001: Function parser interface

## Summary

Add text parsing for function-level IR with a public API centered on
`pipeline.parse(text)`.
Adopt a flat v1 text format with explicit `stage` and `specialize`
declarations, semicolon-terminated stage declarations, strict `@`-prefixed
global symbols (including numeric names like `@1`), and whitespace/comment
agnostic parsing. This RFC intentionally replaces the old ambiguous staged
format and keeps no legacy compatibility mode.

## Motivation

- Problem: Kirin can print function IR but cannot parse function text back with
  matching ergonomics.
- Why now: parser/printer asymmetry blocks textual workflows and increases
  custom glue code.
- Stakeholders:
  - `kirin-chumsky` maintainers
  - `kirin-prettyless` maintainers
  - dialect authors using `HasParser`/`EmitIR`
  - users integrating textual IR tooling

## Goals

- Priority 1: single pipeline parse entrypoint with stage-driven dialect dispatch.
- Priority 2: deterministic, flat syntax with explicit declaration boundaries.
- Remove linebreak-sensitive grammar behavior.
- Keep stage/function symbols uniform as global-symbol syntax (`@...`).
- Compose function parsing with existing dialect body parsers.
- Provide clear parse errors, wrapping chumsky diagnostics.

## Non-goals

- Generic serialization of all runtime metadata.
- Dialect body syntax redesign.
- Backward compatibility for pre-RFC function text syntax.

## Guide-level Explanation

"v1" means the first syntax defined by this RFC.

The syntax is flat:

- `stage` declares a staged-function signature.
- `specialize` declares a specialization body for a specific
  `(stage, function)`.

Examples:

```text
stage @host fn @extern_fn(int) -> float;
```

```text
stage @A fn @foo(any) -> any;
specialize @A fn @foo(int) -> int { ... }
specialize @A fn @foo(float) -> float { ... }
```

```text
stage @1 fn @42(int) -> int;
specialize @1 fn @42(int) -> int { ... }
```

Whitespace and line breaks are insignificant. `//` and `/* ... */` comments are
accepted anywhere whitespace is accepted.

API model:

- Parse on `Pipeline`: parse whole-pipeline text; may contain multiple function
  names.
- Stage container creation is controlled by a stage-container trait, so
  `stage @X ...;` can create missing stages during parse.

## Reference-level Explanation

### API and syntax changes

Public surface is trait-first, using `parse` on pipeline contexts.

```rust
pub trait CompileStageInfo: Sized {
    type Languages;

    fn stage_name(&self) -> Option<GlobalSymbol>;
    fn set_stage_name(&mut self, name: Option<GlobalSymbol>);
    fn stage_id(&self) -> Option<CompileStage>;
    fn set_stage_id(&mut self, id: Option<CompileStage>);
    fn from_stage_name(stage_name: &str) -> Result<Self, String>;
    fn declared_stage_names() -> &'static [&'static str] { &[] }
}

pub trait ParsePipelineText {
    fn parse(&mut self, src: &str) -> Result<Vec<Function>, FunctionParseError>;
}
```

Intended implementations:

- `Pipeline<S>: ParsePipelineText`
- `StageInfo<L>: CompileStageInfo` (default stage creation)
- custom stage enums: `#[derive(CompileStageInfo)]` with explicit symbol mapping and
  registered dialect list in `Languages`

Notes:

- Public API should minimize new names.
- Stage mapping is derive-friendly via `#[derive(CompileStageInfo)]`.

### `#[derive(CompileStageInfo)]`

The `CompileStageInfo` derive macro automates boilerplate for compile-stage
enums. Unlike the dialect-oriented derives (`Dialect`, `HasParser`, etc.) which
use the `kirin-derive-core` IR system (`#[kirin(...)]` attributes, field
classification into arguments/results/regions), this derive targets
**compile-stage definitions** — enums whose variants each wrap a `StageInfo<L>`.
It uses its own `#[stage(...)]` attribute namespace and parses input directly
with `syn`, since stage enums have no IR field categories.

```rust
#[derive(CompileStageInfo)]
enum MixedStage {
    #[stage(name = "parse")]
    Parse(StageInfo<FunctionBody>),
    #[stage(name = "lower")]
    Lower(StageInfo<LowerBody>),
}
```

The macro generates:

- `HasStageInfo<L>` for each unique dialect type (with or-patterns when multiple
  variants share the same dialect).
- `CompileStageInfo` impl: stage identity delegation (`stage_name`,
  `set_stage_name`, `stage_id`, `set_stage_id`), `from_stage_name()` dispatch,
  `declared_stage_names()`, and the `Languages` associated type as a right-folded
  nested tuple (e.g., `(FunctionBody, (LowerBody, ()))`) for dialect tuple
  dispatch used by `ParsePipelineText`.

An optional `#[stage(crate = "...")]` attribute on the enum overrides the
default IR crate path (`::kirin::ir`), useful when deriving inside individual
crates (e.g., `#[stage(crate = "kirin_ir")]`).

### Grammar (v1)

Token grammar (whitespace/comment agnostic):

```text
IdentLike       := Ident | Digits
GlobalSymbol    := "@" IdentLike
StageName       := GlobalSymbol
FnName          := GlobalSymbol

TypeList        := Type ("," Type)*
FnSig           := "fn" FnName "(" TypeList? ")" "->" Type

StageDecl       := "stage" StageName FnSig ";"
SpecializeDecl  := "specialize" StageName FnSig Statement
Decl            := StageDecl | SpecializeDecl
Input           := Decl+
```

Lexical rules:

- comments: `// ...` and `/* ... */`
- whitespace/newlines are insignificant
- declaration boundaries are tokenized (`;` for `stage`, statement parser for
  `specialize` body)

### Semantics and invariants

Shared invariants:

- Stage and function symbols must use global-symbol syntax (`@...`).
- Bare integers like `stage 1 ...` are invalid; use `@1`.
- `stage` declarations require trailing `;`.
- `specialize` declarations must have bodies (no `specialize ...;` form).
- Parser performs structural checks only; type consistency checks are deferred to
  later passes.

Pipeline parse behavior:

- `Pipeline::parse(...)`
  - accepts mixed function names in one input
  - groups/creates abstract functions and staged functions accordingly
  - supports mixed dialects in one text input; stage variant determines dialect
  - `stage @X` creates a missing stage via `CompileStageInfo::from_stage_name`
  - `specialize` must resolve to an existing staged declaration in parse scope
    (declared in the same parse input or already present in target context)

Missing stage declaration behavior:

- Parsing a `specialize` with no resolvable stage declaration is a hard error.

Roundtrip target:

- `print -> parse -> print` text equality, except trailing newline differences.
- Parser does not apply extra normalization.

### Error model

Use a domain error type that wraps chumsky diagnostics.

```rust
pub struct FunctionParseError {
    pub kind: FunctionParseErrorKind,
    pub span: Option<SimpleSpan>,
    pub message: String,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}
```

Guidelines:

- Preserve chumsky expectation/context detail in wrapped errors.
- Use domain kinds for semantic failures (unknown stage, mismatch, missing
  stage declaration, emit failures).
- Closest-stage suggestions are best-effort, using `strsim::levenshtein` for
  candidate ranking.
- `FunctionParseErrorKind` has no stability commitment yet (pre-stable phase).

Implemented kinds:

- `InvalidHeader`
- `UnknownStage`
- `InconsistentFunctionName` (reserved for future function-scoped parse)
- `MissingStageDeclaration`
- `BodyParseFailed`
- `EmitFailed`

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-ir` | `CompileStageInfo` trait, `HasStageInfo<L>`, `Pipeline` stage APIs | staged/specialized construction tests |
| `kirin-derive-dialect` | `stage_info` code generator for `#[derive(CompileStageInfo)]` | — |
| `kirin-derive` | proc-macro entry point for `CompileStageInfo` | — |
| `kirin-chumsky` | v1 grammar, `ParsePipelineText`, dialect dispatch, wrapped errors | positive/negative parser tests |
| `kirin-prettyless` | printer emits flat `stage`/`specialize` syntax | snapshot updates + roundtrip tests |

## Drawbacks

- Syntax change causes immediate snapshot churn.
- No legacy syntax mode may require coordinated updates across downstream tools.
- Structural-only parsing defers some failures to later analysis passes.

## Rationale and Alternatives

### Proposed approach rationale

Flat `stage` + `specialize` declarations keep parsing composable and explicit,
remove delimiter ambiguity, and provide a single pipeline-level parse entry
point without proliferating API names.

### Alternative A: keep nested/implicit staged format

- Description: keep staged blocks with inferred staged signatures.
- Pros: fewer short-term printer edits.
- Cons: repetition, weaker composability, ambiguity pressure around extern/body.
- Reason not chosen: conflicts with composability and explicitness goals.

### Alternative B: compatibility parser for old + new syntax

- Description: accept both syntaxes in v1.
- Pros: easier migration.
- Cons: extra complexity and maintenance branch in parser/printer behavior.
- Reason not chosen: explicit decision to drop legacy syntax and keep one format.

## Prior Art

- Rust tooling generally values deterministic parse/print cycles.
- MLIR-like textual IR practice favors explicit declarations and token-delimited
  grammars over newline-significant forms.

## Backward Compatibility and Migration

- Breaking changes:
  - old staged textual forms are removed
  - stage/function symbols always require `@`
  - stage declarations require `;`
- Migration steps:
  1. implement v1 parser + trait-based API
  2. update pretty printer to emit v1 flat syntax
  3. refresh snapshots and parser tests
  4. re-export parse traits in `kirin-chumsky` prelude
- Compatibility strategy:
  - one supported syntax (v1)
  - downstream updates are expected during rollout

## How to Teach This

- Teach the single parse entry point: `pipeline.parse(text)` accepts
  whole-pipeline text with mixed function names and multiple stages.
- Teach syntax by two declaration forms only: `stage` and `specialize`.
- Document that body parsing is delegated to dialect parser composition.
- Document symbol rule once: global symbols are always `@...`.
- Teach `#[derive(CompileStageInfo)]` for multi-dialect stage enums;
  for single-dialect pipelines, `StageInfo<L>` implements `CompileStageInfo`
  automatically.

## Reference Implementation Plan

1. Define parser AST/tokens for `stage` and `specialize` declarations.
2. Implement whitespace/comment handling compatible with dialect parser
   composition.
3. Implement `ParsePipelineText` trait on `Pipeline<S>`, using the pipeline's
   internal global symbol table for name resolution.
4. Implement pipeline-level semantic checks (unknown stage, missing stage
   declaration, signature mismatch).
5. Wrap chumsky diagnostics in domain parse errors.
6. Update pretty printer to v1 flat syntax.
7. Re-export parse traits from `kirin-chumsky` prelude.
8. Add/refresh parser, roundtrip, and integration tests.
9. Implement `#[derive(CompileStageInfo)]` for stage enum boilerplate.

### Acceptance Criteria

- [x] Parser accepts only v1 flat syntax (`stage` + `specialize`).
- [x] `stage` declarations require trailing `;`.
- [x] `specialize` declarations require bodies.
- [x] Stage/function names require `@` global-symbol syntax.
- [x] Parser is whitespace/newline agnostic and accepts `//` + `/* ... */`.
- [x] Pipeline parse supports multiple function names in one input.
- [x] Missing stage declaration for `specialize` is a hard error.
- [x] `print -> parse -> print` matches except trailing newline differences.
- [x] Parse traits are re-exported from `kirin-chumsky` prelude.
- [x] `#[derive(CompileStageInfo)]` automates stage enum boilerplate.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - benchmark parser performance on snapshot corpus

## Unresolved Questions

- Whether to add explicit performance thresholds as release gates.

## Future Possibilities

- Function-scoped parse entry point that rejects mismatched function names
  (would use the reserved `InconsistentFunctionName` error kind).
- Staged-function-scoped parse for targeted specialization ingestion.
- Re-export parse traits from a top-level `kirin` crate prelude (when a
  top-level library crate is created).
- Fuzz/property tests for parser/printer roundtrip behavior.
- Optional richer serialization format for non-textual metadata.
- Additional parser tooling around migration diagnostics.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-08T03:49:05.889848Z | RFC created from template |
| 2026-02-08T03:50:32.433343Z | Filled draft content from `design/function-parser-interface.md` |
| 2026-02-08T04:31:53Z | Refactored RFC after interview feedback (explicit staged syntax, no ParseConfig, mandatory global symbols) |
| 2026-02-08T06:01:37Z | Incorporated one-by-one interview decisions (flat stage/specialize syntax, trait-first parse API, no legacy syntax, prelude re-export) |
| 2026-02-08T06:04:42Z | Selected `strsim` + Levenshtein for best-effort stage-name hints |
| 2026-02-08T06:07:34.78824Z | RFC status set to `Accepted` |
| 2026-02-08 | Merged `StageIdentity` into `CompileStageInfo`, added `#[derive(CompileStageInfo)]`, aligned RFC with implementation (removed unimplemented function-scoped parse, updated crate matrix and acceptance criteria) |
