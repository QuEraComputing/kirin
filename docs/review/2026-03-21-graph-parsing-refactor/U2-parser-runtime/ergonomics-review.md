# U2: Parser Runtime -- Ergonomics/DX Review

## Toy Scenario

I have a dialect `TweezerOps` with operations like `move_atom`, `raman_pulse`. I want to parse a single statement from text and add it to an existing stage.

```rust
use kirin_chumsky::prelude::*;

let mut stage: StageInfo<TweezerOps> = StageInfo::default();
let stmt = stage.parse_statement("%out = move_atom %src, %dst")?;
```

This works cleanly. The `ParseStatementTextExt` blanket erases the `Ctx = ()` argument. For pipeline usage:

```rust
let stmt = pipeline.parse_statement::<TweezerOps>(stage_id, "%out = move_atom %src, %dst")?;
```

The turbofish `<TweezerOps>` is necessary because `Pipeline<S>` is parameterized by a stage enum, not the dialect. This is an unavoidable consequence of multi-dialect pipelines.

Edge case: What happens if I typo a field name? The error is `EmitError::UndefinedSSA("srcc")` -- the message is `"undefined SSA value: %srcc"`. Clear enough.

Edge case: What happens if I parse a block-containing statement inline? The `ParseEmit` trait requires the dialect to support this. If not, I get a compile error rather than a runtime panic.

## Findings

### [P2] [confirmed] Three implementation paths for ParseEmit create decision paralysis -- parse_emit.rs:62-80

New users face a 3-way choice: (1) `#[derive(HasParser)]`, (2) `SimpleParseEmit` marker, (3) manual `ParseEmit`. The doc comment explains them, but there is no guidance on *when* to pick which. A decision table would help: "Does your dialect have Block/Region fields? -> derive. No Block/Region, no recursion? -> SimpleParseEmit. Custom parse logic? -> manual."

### [P2] [likely] ChumskyError conflates two unrelated error domains -- parse_emit.rs:11-16

`ChumskyError::Parse(Vec<ParseError>)` and `ChumskyError::Emit(EmitError)` are semantically different phases. Users matching on errors must handle both. The name `ChumskyError` is also confusing -- it suggests a chumsky-specific error, but `EmitError` is not chumsky-related. Consider `ParseAndEmitError` or similar.

### [P1] [confirmed] EmitContext forward-reference mode is invisible to users -- emit_ir.rs:83-89

`set_relaxed_dominance(true)` enables forward references in graph bodies, but there is no documentation or example showing when/why a user would call this. The API surface (`resolve_ssa` vs `lookup_ssa`) is also subtle: `lookup_ssa` returns `None` for missing names, `resolve_ssa` creates placeholders. This distinction is critical for graph parsing but not documented at the callsite.

### [P3] [uncertain] parse_ast returns Vec<ParseError> not ChumskyError -- has_parser.rs:108

The free function `parse_ast` returns `Result<L::Output, Vec<ParseError>>`, while `parse_statement` returns `Result<Statement, ChumskyError>`. Users who start with `parse_ast` and later switch to `parse_statement` must update error handling. Minor but inconsistent.

## Concept Budget Table

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| HasParser<'t> / HasDialectParser<'t> | has_parser.rs | Med |
| ParseEmit (3 paths) | parse_emit.rs | High |
| EmitIR / EmitContext | emit_ir.rs | Med |
| ParseStatementText / ParseStatementTextExt | parse_text.rs | Low |
| ChumskyError (Parse vs Emit) | parse_emit.rs | Med |
| DirectlyParsable marker | emit_ir.rs | Low |
| Forward-reference / relaxed dominance | emit_ir.rs | High |

## Lifetime Complexity

(i) **Hidden by derive**: `HasParser<'t>` lifetime hidden behind `#[derive(HasParser)]` -- users never write it.
(ii) **Visible necessary**: `EmitContext<'a, L>` borrows `BuilderStageInfo` -- required for mutation.
(iii) **Visible avoidable**: None found. Single-lifetime design is clean.

## Strengths

- `ParseStatementTextExt` erasing the `Ctx = ()` is elegant -- `stage.parse_statement(input)` just works.
- `DirectlyParsable` marker for identity emit is a nice touch -- type lattices parse to themselves without boilerplate.
- Error types are well-structured with clear Display impls.
- `parse_ast` as a free function is a clean entry point for AST-only workflows.
