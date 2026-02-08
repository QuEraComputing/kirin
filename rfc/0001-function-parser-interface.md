+++
rfc = "0001"
title = "Function parser interface"
status = "Accepted"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-08T03:49:05.889848Z"
last_updated = "2026-02-08T06:07:34.78824Z"
+++

# RFC 0001: Function parser interface

## Summary

Add text parsing for function-level IR with a public API that is trait-based and
entry-point-driven (`parse` on pipeline/function/staged-function contexts).
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

- Priority 1: dual API model, expressed as `parse` on the target IR context.
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
- Parse on `Function`: parse text for one known abstract function; mismatched
  function names are errors.
- Parse on `StagedFunction`: parse specialization text for one known staged
  function context.

## Reference-level Explanation

### API and syntax changes

Public surface is trait-first, using `parse` methods on IR contexts.

```rust
pub trait ParseText {
    type Output;

    fn parse(
        &mut self,
        src: &str,
        global_symbols: &mut InternTable<String, GlobalSymbol>,
    ) -> Result<Self::Output, FunctionParseError>;
}
```

Intended implementations:

- `Pipeline<S>: ParseText`
- function-scoped context (for known abstract function): `ParseText`
- staged-function-scoped context (for known staged function): `ParseText`

Notes:

- Public API should minimize new names. Entry-point type determines behavior.
- Any parser helper structs may exist internally, but they are not required as
  public API.
- Global symbol table input is explicit at parse call sites.

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

Entry-point-specific behavior:

- `Pipeline::parse(...)`
  - accepts mixed function names in one input
  - groups/creates abstract functions and staged functions accordingly
  - `specialize` must resolve to an existing staged declaration in parse scope
    (declared in the same parse input or already present in target context)
- function-scoped `parse(...)`
  - function name is fixed by context
  - mismatched function names are hard errors
- staged-function-scoped `parse(...)`
  - stage + function symbols are fixed by context
  - `specialize` must match stage/function symbols
  - intended for composable specialization ingestion

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

Representative kinds:

- `InvalidHeader`
- `UnknownStage`
- `InconsistentFunctionName`
- `MissingStageDeclaration`
- `BodyParseFailed`
- `EmitFailed`

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-ir` | helper APIs for staged/specialized insertion paths may be needed | staged/specialized construction tests |
| `kirin-chumsky` | v1 grammar, parser composition with dialect body parser, wrapped errors | positive/negative parser tests |
| `kirin-prettyless` | printer emits flat `stage`/`specialize` syntax | snapshot updates + roundtrip tests |
| `kirin` | re-export parse traits in prelude (first PR) | top-level integration tests |

## Drawbacks

- Syntax change causes immediate snapshot churn.
- No legacy syntax mode may require coordinated updates across downstream tools.
- Structural-only parsing defers some failures to later analysis passes.

## Rationale and Alternatives

### Proposed approach rationale

Flat `stage` + `specialize` declarations keep parsing composable and explicit,
remove delimiter ambiguity, and align with the requirement to parse via context
(`pipeline` vs `function` vs `staged function`) without proliferating API names.

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
  4. re-export parse traits in top-level prelude
- Compatibility strategy:
  - one supported syntax (v1)
  - downstream updates are expected during rollout

## How to Teach This

- Teach parse entry by context type:
  - pipeline parse: whole input, multi-function allowed
  - function parse: single-function constrained
  - staged-function parse: specialization ingestion
- Teach syntax by two declaration forms only: `stage` and `specialize`.
- Document that body parsing is delegated to dialect parser composition.
- Document symbol rule once: global symbols are always `@...`.

## Reference Implementation Plan

1. Define parser AST/tokens for `stage` and `specialize` declarations.
2. Implement whitespace/comment handling compatible with dialect parser
   composition.
3. Implement trait-based parse entry points with explicit symbol-table argument.
4. Implement entry-point-specific semantic checks.
5. Wrap chumsky diagnostics in domain parse errors.
6. Update pretty printer to v1 flat syntax.
7. Re-export parse traits from top-level prelude.
8. Add/refresh parser, roundtrip, and integration tests.

### Acceptance Criteria

- [ ] Parser accepts only v1 flat syntax (`stage` + `specialize`).
- [ ] `stage` declarations require trailing `;`.
- [ ] `specialize` declarations require bodies.
- [ ] Stage/function names require `@` global-symbol syntax.
- [ ] Parser is whitespace/newline agnostic and accepts `//` + `/* ... */`.
- [ ] Pipeline parse supports multiple function names in one input.
- [ ] Function-scoped parse rejects mismatched function names.
- [ ] Missing stage declaration for `specialize` is a hard error.
- [ ] `print -> parse -> print` matches except trailing newline differences.
- [ ] Parse traits are re-exported from top-level prelude.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - benchmark parser performance on snapshot corpus

## Unresolved Questions

- Exact function/staged-function parse context types and signatures.
- Whether to add explicit performance thresholds as release gates.

## Future Possibilities

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
