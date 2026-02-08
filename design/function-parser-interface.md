# Function Parser Interface (Dual to Pretty Printing)

## Status

Draft design note for implementing default text parsers for:

- `Function` (pipeline-wide)
- `StagedFunction` (single stage)
- `SpecializedFunction` (single stage)

Goal: roundtrip with the **current** pretty-printed function text format used in
`crates/kirin-prettyless/src/tests/snapshots`.

## Motivation

`kirin-prettyless` already provides human-readable function printing:

- stage-local via `Document::print_specialized_function` and `Document::print_staged_function`
- pipeline-wide via `FunctionPrintExt` (`Function::sprint(&pipeline)`)

We need the parser side to be structurally dual:

- if we have `print_*`, provide `parse_*`
- if print methods consume `&StageInfo`/`&Pipeline`, parse methods consume
  `&mut StageInfo`/`&mut Pipeline`
- roundtrip target: `print -> parse -> print` with the same textual form
  (modulo trailing newline normalization)

## Current Printed Syntax (v1)

From current snapshots, the concrete syntax is:

### Specialized function (single stage context)

```text
fn @name(T0, T1) -> Ret { ...body statement... }
```

### Staged function with stage identity

```text
stage @A fn @name(T0) -> Ret { ...body statement... }
stage 0 fn @name(T0) -> Ret { ...body statement... }
```

### Extern / declaration-only staged function

```text
stage @host fn @extern_fn(int) -> float
```

### Pipeline function rendering

Multiple staged renderings separated by blank lines, all for one function name:

```text
stage @A fn @foo(int) -> int { ... }

stage @B fn @foo(int) -> int { ... }
```

## Dual API Principle

We keep API naming dual to existing print surfaces.

| Pretty side | Parser side (proposed) | Context |
|---|---|---|
| `Document::print_specialized_function` | `FunctionParser::parse_specialized_function` | `&mut StageInfo<L>` |
| `Document::print_staged_function` | `FunctionParser::parse_staged_function` | `&mut StageInfo<L>` |
| `FunctionPrintExt::sprint(&Pipeline)` | `FunctionParseExt::parse_function(&mut Pipeline)` | `&mut Pipeline<S>` |

Notes:

- parser methods consume source text and mutate IR context
- print methods consume IR context and produce text
- pipeline-level parse requires pipeline input exactly like pipeline-level print

## Proposed Public Interfaces

## Stage-local parser context

```rust
pub struct FunctionParser<'a, L: Dialect> {
    // required for allocation/emission
    stage: &'a mut StageInfo<L>,
    // optional for resolving/interning function and stage names
    global_symbols: Option<&'a mut InternTable<String, GlobalSymbol>>,
    config: ParseConfig,
}

impl<'a, L> FunctionParser<'a, L>
where
    L: Dialect + HasParser<'a, 'a>,
    L::Type: HasParser<'a, 'a, Output = L::Type>,
{
    pub fn new(stage: &'a mut StageInfo<L>) -> Self;
    pub fn with_global_symbols(
        stage: &'a mut StageInfo<L>,
        global_symbols: &'a mut InternTable<String, GlobalSymbol>,
    ) -> Self;

    pub fn parse_specialized_function(
        &mut self,
        src: &str,
    ) -> Result<SpecializedFunction, FunctionParseError>;

    pub fn parse_staged_function(
        &mut self,
        src: &str,
    ) -> Result<StagedFunction, FunctionParseError>;
}
```

## Pipeline parser context

```rust
pub struct PipelineParser<'a, S> {
    pipeline: &'a mut Pipeline<S>,
    config: ParseConfig,
}

impl<'a, S: ParseStage> PipelineParser<'a, S> {
    pub fn new(pipeline: &'a mut Pipeline<S>) -> Self;
    pub fn parse_function(&mut self, src: &str) -> Result<Function, FunctionParseError>;
}
```

## Parse trait dual to `FunctionPrintExt`

```rust
pub trait FunctionParseExt {
    fn parse_function<S: ParseStage>(
        &self,
        pipeline: &mut Pipeline<S>,
    ) -> Result<Function, FunctionParseError>;

    fn parse_function_with_config<S: ParseStage>(
        &self,
        config: ParseConfig,
        pipeline: &mut Pipeline<S>,
    ) -> Result<Function, FunctionParseError>;
}

impl FunctionParseExt for str { ... }
```

Rationale:

- keeps callsite ergonomic and visually dual:
  - print: `func.sprint(&pipeline)`
  - parse: `text.parse_function(&mut pipeline)`

## Trait dual to `RenderStage`

Pipeline printing uses type-erased `RenderStage`. Parsing needs the inverse:

```rust
pub trait ParseStage {
    fn parse_staged_function(
        &mut self,
        src: &str,
        config: &ParseConfig,
        global_symbols: &mut InternTable<String, GlobalSymbol>,
    ) -> Result<Option<StagedFunction>, FunctionParseError>;
}
```

Blanket implementation is provided for `StageInfo<L>` when `L` supports
statement parsing/emission.

## Parse Model

Parser remains two-phase:

1. Parse text into lightweight function AST/header structures.
2. Emit IR using existing builders:
   - `stage.staged_function()...new()`
   - `stage.specialize()...new()`
   - statement body emission via existing `parse_ast::<L>` + `EmitIR`.

This keeps function parsing aligned with current statement parsing architecture.

## Header Grammar (v1, compact form)

```text
StagePrefix     := "stage" ( "@" Ident | UInt )
FnHeader        := [StagePrefix] "fn" "@" Ident "(" TypeList? ")" "->" Type
SpecDecl        := FnHeader Statement
ExternDecl      := FnHeader
StagedDeclText  := ExternDecl | SpecDecl (BlankLine+ SpecDecl)*
FunctionText    := StagedDeclText (BlankLine+ StagedDeclText)*
```

`Statement` uses the existing dialect parser (`L::parser()`).

## Semantics and Grouping

### `parse_specialized_function`

- parses one `FnHeader + Statement`
- creates/looks up parent staged function in current stage
- emits body statement and creates one specialization

### `parse_staged_function`

- parses one staged chunk in current stage
- accepts either:
  - extern declaration (header only)
  - one or more specialization declarations
- if multiple specialization declarations are present:
  - staged signature is inferred (see below)

### `parse_function` (pipeline)

- parses multiple staged chunks
- requires stage prefix per chunk for routing
- resolves route by stage name (`stage @A`) or stage id (`stage 0`)
- creates one `Function` in pipeline and links parsed staged functions
- enforces single function name across all chunks

## Staged Signature Inference

Current printed format does not explicitly print staged signature when multiple
specializations exist; only specialization headers are visible.

For parser default behavior:

- 0 specialization (extern): staged signature = header signature
- 1 specialization: staged signature = that signature
- N specializations: staged signature inferred by pointwise join over parameter
  and return types

This preserves parse ability for current syntax. Text roundtrip remains stable
because the staged signature is not printed today.

## ParseConfig

```rust
pub struct ParseConfig {
    pub require_stage_prefix_for_pipeline: bool, // default: true
    pub allow_stage_prefix_in_stage_parser: bool, // default: true
    pub strict_stage_identity_match: bool, // default: true
    pub allow_unnamed_function_token: bool, // default: true (`@<unnamed>`)
    pub infer_staged_signature: bool, // default: true
}
```

## Error Model

```rust
pub struct FunctionParseError {
    pub kind: FunctionParseErrorKind,
    pub span: Option<SimpleSpan>,
    pub message: String,
}
```

Representative error kinds:

- `InvalidHeader`
- `UnknownStageByName`
- `UnknownStageById`
- `StagePrefixMismatch`
- `InconsistentFunctionName`
- `BodyParseFailed`
- `EmitFailed`
- `SignatureInferenceFailed`

## Roundtrip Guarantees

For outputs produced by current printers:

- `Document::print_specialized_function` text should parse with
  `parse_specialized_function`.
- `Document::print_staged_function` text should parse with
  `parse_staged_function`.
- `Function::sprint(&pipeline)` text should parse with `parse_function`.

Expected equality target:

- normalized textual equality after re-print (ignore trailing newline differences)

Known non-roundtrippable metadata in v1:

- invalidated flags
- backedges
- arena IDs
- exact staged signature when not uniquely recoverable from printed specializations

## Phased Implementation

1. Header AST + header parser (stage prefix, function name, signature).
2. `parse_specialized_function` for `StageInfo<L>`.
3. `parse_staged_function` extern + single specialization.
4. `parse_staged_function` multi-specialization + staged-signature inference.
5. Pipeline parser (`ParseStage`, `PipelineParser`, `FunctionParseExt for str`).
6. Snapshot-based roundtrip tests against current `kirin-prettyless` outputs.

## Test Plan

- parser accepts all current snapshots in
  `crates/kirin-prettyless/src/tests/snapshots`.
- roundtrip snapshots:
  - `print -> parse -> print` equality for specialized/staged/pipeline cases.
- negative tests:
  - mixed function names in one pipeline text
  - unknown stage name/id
  - malformed header and malformed signature
  - extern declaration with unexpected trailing body tokens
