//! Stage-dispatched pipeline parser for textual function declarations.
//!
//! This module parses a sequence of `stage` and `specialize` declarations into
//! a [`Pipeline`](kirin_ir::Pipeline). Each declaration starts with a symbolic
//! stage (for example `@A`), and the concrete dialect parser is chosen at
//! runtime by dispatching on that stage's actual `StageInfo<L>` variant.
//!
//! ## Implementation idea
//!
//! Parsing is intentionally split into **two passes**:
//!
//! 1. **Pass 1 (headers + indexing)**
//!    - parse one declaration at a time using `dispatch_stage_mut`;
//!    - materialize/validate `stage` declarations immediately;
//!    - collect `(stage, function) -> staged_function` mappings;
//!    - record offsets of `specialize` declarations for pass 2.
//!
//! 2. **Pass 2 (specialize bodies)**
//!    - re-parse only the previously recorded `specialize` declarations;
//!    - resolve the target staged function from the pass-1 lookup;
//!    - emit specialization bodies into the resolved stage dialect.
//!
//! This separation guarantees that specialization emission sees a complete
//! staged-function header set, which keeps behavior deterministic even when
//! declarations are interleaved across stages.
//!
//! ## Why stage dispatch is central
//!
//! A pipeline can contain different dialects per stage (for example stage `A`
//! with `FunctionBody`, stage `B` with `LowerBody`). The parser does not guess
//! which dialect to use from text alone. Instead it:
//!
//! - resolves/creates the stage symbol first (`@A`, `@B`, ...);
//! - uses generic stage dispatch to select the matching `L`;
//! - runs `parse_one_declaration::<L>` and emit logic under that `L`.
//!
//! If a stage exists but its dialect is not in `S::Languages`, dispatch returns
//! a dialect-miss error.
//!
//! ## Illustrative examples
//!
//! Same-stage header + body:
//!
//! ```text
//! stage @A fn @foo(()) -> ();
//! specialize @A fn @foo(()) -> () { ^0() {} }
//! ```
//!
//! - Pass 1 creates/finds function `@foo` and staged function `(A, foo)`.
//! - Pass 2 emits the specialize body into stage `A`.
//!
//! Mixed-stage dialect dispatch:
//!
//! ```text
//! stage @A fn @foo(()) -> ();
//! specialize @A fn @foo(()) -> () { ^0() {} }
//! stage @B fn @bar(i32) -> i32;
//! specialize @B fn @bar(i32) -> i32 { ^0() {} }
//! ```
//!
//! - declarations for `@A` are parsed with stage `A`'s dialect;
//! - declarations for `@B` are parsed with stage `B`'s dialect.
//!
//! Missing header before specialize:
//!
//! ```text
//! specialize @A fn @missing(()) -> () { ^0() {} }
//! ```
//!
//! - pass 2 cannot find `(A, missing)` in the staged lookup;
//! - returns `MissingStageDeclaration`.
//!
//! ## Data flow summary
//!
//! - `staged_lookup`: stable key map for staged-function resolution across passes.
//! - `function_lookup`: name-to-function cache to avoid repeated arena scans.
//! - `pending_specializations`: source offsets to re-dispatch specialize bodies.
//! - `ParseState`: deduplicated set of touched abstract functions returned to caller.
//!
use std::collections::{HashMap, HashSet};

use chumsky::span::SimpleSpan;
use kirin_ir::{
    CompileStage, Dialect, Function, GetInfo, GlobalSymbol, HasStageInfo, Id, Pipeline,
    StageActionMut, StageDispatchMiss, StageDispatchRequiredError, StageInfo, StageMeta,
    StagedFunction, Statement, SupportsStageDispatchMut,
};
use kirin_lexer::Token;
use strsim::levenshtein;

use crate::{EmitContext, EmitIR, HasParser};

use super::error::{DiagnosticError, FunctionParseError, FunctionParseErrorKind};
use super::syntax::{ChumskyError, Declaration, Header, parse_one_declaration, tokenize};
use crate::ast::SymbolName;

/// Parse function text into a pipeline using stage-driven dialect dispatch.
pub trait ParsePipelineText {
    fn parse(&mut self, src: &str) -> Result<Vec<Function>, FunctionParseError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct StagedKey {
    stage: CompileStage,
    function: Function,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeclKeyword {
    Stage,
    Specialize,
}

#[derive(Debug, Clone)]
struct DeclarationHead<'src> {
    keyword: DeclKeyword,
    stage: SymbolName<'src>,
    function: SymbolName<'src>,
}

#[derive(Clone, Copy, Debug)]
struct FirstPassOutcome {
    keyword: DeclKeyword,
    next_index: usize,
}

#[derive(Clone, Copy, Debug)]
struct FirstPassDispatchResult {
    outcome: FirstPassOutcome,
    link: Option<(Function, StagedFunction)>,
}

/// Shared mutable state threaded through both parse passes.
struct ParseState {
    touched_functions: Vec<Function>,
    touched_function_set: HashSet<Function>,
}

impl ParseState {
    fn new() -> Self {
        Self {
            touched_functions: Vec::new(),
            touched_function_set: HashSet::new(),
        }
    }

    fn record(&mut self, function: Function) {
        if self.touched_function_set.insert(function) {
            self.touched_functions.push(function);
        }
    }
}

struct FirstPassAction<'a, 'src> {
    tokens: &'a [(Token<'src>, SimpleSpan)],
    start_index: usize,
    function: Option<Function>,
    function_symbol: Option<GlobalSymbol>,
    staged_lookup: &'a mut HashMap<StagedKey, StagedFunction>,
    state: &'a mut ParseState,
}

impl<'src, S, L> StageActionMut<S, L> for FirstPassAction<'_, 'src>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    for<'tokens> L: HasParser<'tokens, 'tokens>,
    for<'tokens> L::Type: HasParser<'tokens, 'tokens, Output = L::Type>,
    for<'tokens> <L as HasParser<'tokens, 'tokens>>::Output: EmitIR<L, Output = Statement>,
{
    type Output = FirstPassDispatchResult;
    type Error = FunctionParseError;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &mut StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        // Pass 1 only registers `stage` declarations and records where
        // `specialize` declarations occur. This guarantees all staged-function
        // headers exist before any specialize body is emitted.
        let (declaration, consumed_span) =
            parse_one_declaration::<L>(&self.tokens[self.start_index..])
                .map_err(parse_error_from_chumsky)?;
        let next_index = advance_to_next_declaration(self.tokens, self.start_index, consumed_span);

        match declaration {
            Declaration::Stage(header) => {
                let function = self
                    .function
                    .expect("stage declaration should have a resolved function");
                let function_symbol = self
                    .function_symbol
                    .expect("stage declaration should have a function symbol");
                let staged_function = apply_stage_declaration::<L>(
                    stage,
                    stage_id,
                    function,
                    function_symbol,
                    &header,
                    self.staged_lookup,
                    self.state,
                )?;
                Ok(FirstPassDispatchResult {
                    outcome: FirstPassOutcome {
                        keyword: DeclKeyword::Stage,
                        next_index,
                    },
                    link: staged_function.map(|staged| (function, staged)),
                })
            }
            Declaration::Specialize { .. } => Ok(FirstPassDispatchResult {
                outcome: FirstPassOutcome {
                    keyword: DeclKeyword::Specialize,
                    next_index,
                },
                link: None,
            }),
        }
    }
}

struct SecondPassSpecializeAction<'a, 'src> {
    tokens: &'a [(Token<'src>, SimpleSpan)],
    start_index: usize,
    function_lookup: &'a HashMap<String, Function>,
    staged_lookup: &'a HashMap<StagedKey, StagedFunction>,
    state: &'a mut ParseState,
}

impl<'src, S, L> StageActionMut<S, L> for SecondPassSpecializeAction<'_, 'src>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect + HasParser<'src, 'src>,
    L::Type: HasParser<'src, 'src, Output = L::Type>,
    <L as HasParser<'src, 'src>>::Output: EmitIR<L, Output = Statement>,
{
    type Output = usize;
    type Error = FunctionParseError;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &mut StageInfo<L>,
    ) -> Result<Self::Output, Self::Error> {
        // Pass 2 re-parses only pending specialize declarations once pass 1 has
        // built a complete `(stage, function) -> staged_function` lookup.
        let (declaration, consumed_span) =
            parse_one_declaration::<L>(&self.tokens[self.start_index..])
                .map_err(parse_error_from_chumsky)?;
        let next_index = advance_to_next_declaration(self.tokens, self.start_index, consumed_span);

        let Declaration::Specialize { header, body, span } = declaration else {
            return Err(FunctionParseError::new(
                FunctionParseErrorKind::InvalidHeader,
                Some(consumed_span),
                "expected specialize declaration",
            ));
        };

        apply_specialize_declaration::<L>(
            stage,
            stage_id,
            &header,
            &body,
            span,
            self.function_lookup,
            self.staged_lookup,
            self.state,
        )?;

        Ok(next_index)
    }
}

impl<S> ParsePipelineText for Pipeline<S>
where
    S: StageMeta,
    for<'a, 'src> S: SupportsStageDispatchMut<
            FirstPassAction<'a, 'src>,
            FirstPassDispatchResult,
            FunctionParseError,
        >,
    for<'a, 'src> S:
        SupportsStageDispatchMut<SecondPassSpecializeAction<'a, 'src>, usize, FunctionParseError>,
{
    fn parse(&mut self, src: &str) -> Result<Vec<Function>, FunctionParseError> {
        let tokens = tokenize(src);
        if tokens.is_empty() {
            return Err(FunctionParseError::new(
                FunctionParseErrorKind::InvalidHeader,
                None,
                "expected at least one declaration",
            ));
        }

        let mut staged_lookup = collect_staged_lookup(self);
        let mut function_lookup = collect_function_lookup(self);
        let mut state = ParseState::new();
        // We intentionally defer specialize bodies to pass 2 so forward
        // references like `specialize @A fn @foo ...` before `stage @A fn @foo`
        // are validated against the full header set.
        let mut pending_specializations: Vec<(usize, CompileStage, SymbolName<'_>)> = Vec::new();

        let mut index = 0;
        while index < tokens.len() {
            let head = parse_declaration_head(&tokens, index)?;
            let stage_id = resolve_or_create_stage_symbol(self, &head.stage)?;
            let (function, function_symbol) = if matches!(head.keyword, DeclKeyword::Stage) {
                let function =
                    get_or_create_function_by_name(self, &mut function_lookup, head.function.name);
                (Some(function), Some(function_symbol(self, function)))
            } else {
                (None, None)
            };

            let mut action = FirstPassAction {
                tokens: &tokens,
                start_index: index,
                function,
                function_symbol,
                staged_lookup: &mut staged_lookup,
                state: &mut state,
            };
            let dispatch =
                dispatch_stage_action_required(self, stage_id, &head.stage, &mut action)?;
            let outcome = dispatch.outcome;
            if let Some((function, staged_function)) = dispatch.link {
                self.link(function, stage_id, staged_function);
            }

            if outcome.keyword != head.keyword {
                return Err(FunctionParseError::new(
                    FunctionParseErrorKind::InvalidHeader,
                    Some(head.stage.span),
                    "declaration keyword mismatch while parsing",
                ));
            }
            ensure_forward_progress(
                outcome.next_index,
                index,
                head.stage.span,
                "failed to advance while parsing declaration",
            )?;

            if matches!(outcome.keyword, DeclKeyword::Specialize) {
                pending_specializations.push((index, stage_id, head.stage));
            }

            index = outcome.next_index;
        }

        for (start_index, stage_id, stage_symbol) in pending_specializations {
            let mut action = SecondPassSpecializeAction {
                tokens: &tokens,
                start_index,
                function_lookup: &function_lookup,
                staged_lookup: &staged_lookup,
                state: &mut state,
            };
            let next_index =
                dispatch_stage_action_required(self, stage_id, &stage_symbol, &mut action)?;
            ensure_forward_progress(
                next_index,
                start_index,
                stage_symbol.span,
                "failed to advance while parsing specialize declaration",
            )?;
        }

        Ok(state.touched_functions)
    }
}

fn parse_declaration_head<'src>(
    tokens: &[(Token<'src>, SimpleSpan)],
    start_index: usize,
) -> Result<DeclarationHead<'src>, FunctionParseError> {
    let Some((keyword, keyword_span)) = tokens.get(start_index) else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            None,
            "expected declaration",
        ));
    };

    let keyword = match keyword {
        Token::Identifier("stage") => DeclKeyword::Stage,
        Token::Identifier("specialize") => DeclKeyword::Specialize,
        _ => {
            return Err(FunctionParseError::new(
                FunctionParseErrorKind::InvalidHeader,
                Some(*keyword_span),
                "expected declaration starting with 'stage' or 'specialize'",
            ));
        }
    };

    let Some((stage_symbol, stage_span)) = tokens.get(start_index + 1) else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            Some(*keyword_span),
            "expected stage symbol after declaration keyword",
        ));
    };

    let Token::Symbol(stage_name) = stage_symbol else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            Some(*stage_span),
            "stage names must use global-symbol syntax (e.g., @A)",
        ));
    };

    let Some((fn_keyword, fn_span)) = tokens.get(start_index + 2) else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            Some(*stage_span),
            "expected 'fn' after stage symbol",
        ));
    };
    let Token::Identifier("fn") = fn_keyword else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            Some(*fn_span),
            "expected 'fn' before function symbol",
        ));
    };

    let Some((function_symbol, function_span)) = tokens.get(start_index + 3) else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            Some(*fn_span),
            "expected function symbol after 'fn'",
        ));
    };
    let Token::Symbol(function_name) = function_symbol else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            Some(*function_span),
            "function names must use global-symbol syntax (e.g., @foo)",
        ));
    };

    Ok(DeclarationHead {
        keyword,
        stage: SymbolName {
            name: stage_name,
            span: *stage_span,
        },
        function: SymbolName {
            name: function_name,
            span: *function_span,
        },
    })
}

fn advance_to_next_declaration<'src>(
    tokens: &[(Token<'src>, SimpleSpan)],
    start_index: usize,
    consumed_span: SimpleSpan,
) -> usize {
    let mut index = start_index;
    while index < tokens.len() && tokens[index].1.start < consumed_span.end {
        index += 1;
    }
    if index == start_index {
        return start_index.saturating_add(1);
    }
    index
}

fn apply_stage_declaration<'src, L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    function: Function,
    function_symbol: GlobalSymbol,
    header: &Header<'src, L::Type>,
    staged_lookup: &mut HashMap<StagedKey, StagedFunction>,
    state: &mut ParseState,
) -> Result<Option<StagedFunction>, FunctionParseError>
where
    L: Dialect,
{
    state.record(function);

    let key = StagedKey {
        stage: stage_id,
        function,
    };
    if let Some(existing) = staged_lookup.get(&key).copied() {
        ensure_staged_signature_matches::<L>(stage, existing, header)?;
        return Ok(None);
    }

    let staged_function = stage
        .staged_function()
        .name(function_symbol)
        .signature(header.signature.clone())
        .new()
        .map_err(|err| {
            FunctionParseError::new(
                FunctionParseErrorKind::EmitFailed,
                Some(header.span),
                err.to_string(),
            )
        })?;
    staged_lookup.insert(key, staged_function);

    Ok(Some(staged_function))
}

fn apply_specialize_declaration<'src, L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    header: &Header<'src, L::Type>,
    body: &<L as HasParser<'src, 'src>>::Output,
    span: SimpleSpan,
    function_lookup: &HashMap<String, Function>,
    staged_lookup: &HashMap<StagedKey, StagedFunction>,
    state: &mut ParseState,
) -> Result<(), FunctionParseError>
where
    L: Dialect + HasParser<'src, 'src>,
    L::Type: HasParser<'src, 'src, Output = L::Type>,
    <L as HasParser<'src, 'src>>::Output: EmitIR<L, Output = Statement>,
{
    let (function, staged_function) =
        resolve_specialize_target::<L>(stage_id, header, span, function_lookup, staged_lookup)?;

    let body_statement = {
        let mut emit_ctx = EmitContext::new(stage);
        body.emit(&mut emit_ctx)
    };

    stage
        .specialize()
        .f(staged_function)
        .signature(header.signature.clone())
        .body(body_statement)
        .new()
        .map_err(|err| {
            FunctionParseError::new(
                FunctionParseErrorKind::EmitFailed,
                Some(span),
                err.to_string(),
            )
        })?;

    state.record(function);
    Ok(())
}

fn resolve_specialize_target<'src, L>(
    stage_id: CompileStage,
    header: &Header<'src, L::Type>,
    span: SimpleSpan,
    function_lookup: &HashMap<String, Function>,
    staged_lookup: &HashMap<StagedKey, StagedFunction>,
) -> Result<(Function, StagedFunction), FunctionParseError>
where
    L: Dialect,
{
    let Some(function) = function_lookup.get(header.function.name).copied() else {
        return Err(missing_stage_declaration_error(header, Some(span)));
    };
    let key = StagedKey {
        stage: stage_id,
        function,
    };
    let Some(staged_function) = staged_lookup.get(&key).copied() else {
        return Err(missing_stage_declaration_error(header, Some(span)));
    };
    Ok((function, staged_function))
}

fn dispatch_stage_action_required<'src, S, A, R>(
    pipeline: &mut Pipeline<S>,
    stage_id: CompileStage,
    stage_symbol: &SymbolName<'src>,
    action: &mut A,
) -> Result<R, FunctionParseError>
where
    S: StageMeta + SupportsStageDispatchMut<A, R, FunctionParseError>,
{
    pipeline
        .dispatch_stage_mut_required(stage_id, action)
        .map_err(|error| stage_dispatch_error(error, stage_symbol.name, Some(stage_symbol.span)))
}

fn ensure_forward_progress(
    next_index: usize,
    start_index: usize,
    span: SimpleSpan,
    message: &'static str,
) -> Result<(), FunctionParseError> {
    if next_index > start_index {
        return Ok(());
    }
    Err(FunctionParseError::new(
        FunctionParseErrorKind::InvalidHeader,
        Some(span),
        message,
    ))
}

fn parse_error_from_chumsky(errors: Vec<ChumskyError<'_>>) -> FunctionParseError {
    let diagnostics: Vec<String> = errors.iter().map(ToString::to_string).collect();
    let span = errors.first().map(|error| *error.span());
    let message = diagnostics
        .first()
        .cloned()
        .unwrap_or_else(|| "failed to parse declarations".to_string());
    FunctionParseError::new(FunctionParseErrorKind::InvalidHeader, span, message)
        .with_source(DiagnosticError::new(diagnostics))
}

/// Build a `function-name -> function` lookup from existing pipeline state.
fn collect_function_lookup<S>(pipeline: &Pipeline<S>) -> HashMap<String, Function> {
    let mut lookup = HashMap::new();
    for info in pipeline.function_arena().iter() {
        let function = Function::from(info.clone().unwrap());
        let Some(symbol) = info.name() else {
            continue;
        };
        let Some(name) = pipeline.resolve(symbol) else {
            continue;
        };
        lookup.insert(name.to_string(), function);
    }
    lookup
}

/// Resolve an abstract function by name, creating it if it does not exist.
fn get_or_create_function_by_name<S>(
    pipeline: &mut Pipeline<S>,
    function_lookup: &mut HashMap<String, Function>,
    name: &str,
) -> Function {
    if let Some(existing) = function_lookup.get(name).copied() {
        return existing;
    }
    let function = pipeline.function().name(name.to_string()).new();
    function_lookup.insert(name.to_string(), function);
    function
}

fn function_symbol<S>(pipeline: &Pipeline<S>, function: Function) -> GlobalSymbol {
    pipeline
        .function_info(function)
        .and_then(|info| info.name())
        .expect("stage declarations should always use named functions")
}

/// Build a `(stage, function) -> staged function` lookup from existing pipeline state.
fn collect_staged_lookup<S>(pipeline: &Pipeline<S>) -> HashMap<StagedKey, StagedFunction> {
    let mut lookup = HashMap::new();
    for info in pipeline.function_arena().iter() {
        let function = Function::from(info.clone().unwrap());
        for (&stage, &staged_function) in info.staged_functions() {
            lookup.insert(StagedKey { stage, function }, staged_function);
        }
    }
    lookup
}

/// Resolve a stage symbol to a stage ID, creating a named stage when missing.
fn resolve_or_create_stage_symbol<'src, S>(
    pipeline: &mut Pipeline<S>,
    stage_symbol: &SymbolName<'src>,
) -> Result<CompileStage, FunctionParseError>
where
    S: StageMeta,
{
    if let Some(stage_id) = find_stage_symbol(pipeline, stage_symbol.name) {
        return Ok(stage_id);
    }

    let stage = S::from_stage_name(stage_symbol.name).map_err(|message| {
        FunctionParseError::new(
            FunctionParseErrorKind::UnknownStage,
            Some(stage_symbol.span),
            stage_creation_error_message::<S>(pipeline, stage_symbol.name, message),
        )
    })?;

    Ok(pipeline
        .add_stage()
        .stage(stage)
        .name(stage_symbol.name.to_string())
        .new())
}

fn stage_creation_error_message<S>(
    pipeline: &Pipeline<S>,
    stage_symbol: &str,
    message: String,
) -> String
where
    S: StageMeta,
{
    let mut output = message;
    let mut candidates = stage_candidates(pipeline);
    for name in S::declared_stage_names() {
        candidates.push((*name).to_string());
    }
    candidates.sort();
    candidates.dedup();

    if let Some(suggestion) = best_stage_suggestion(stage_symbol, &candidates) {
        if !output.contains(&suggestion) {
            output.push_str(&format!(", did you mean '@{suggestion}'?"));
        }
    }
    output
}

/// Lookup a stage by symbolic name (`@A`) or numeric symbol (`@1`).
fn find_stage_symbol<S>(pipeline: &Pipeline<S>, stage_symbol: &str) -> Option<CompileStage>
where
    S: StageMeta,
{
    for stage in pipeline.stages() {
        if let Some(name) = stage
            .stage_name()
            .and_then(|symbol| pipeline.resolve(symbol).map(str::to_string))
        {
            if name == stage_symbol {
                if let Some(stage_id) = stage.stage_id() {
                    return Some(stage_id);
                }
            }
        }
    }

    if let Ok(raw_id) = stage_symbol.parse::<usize>() {
        for stage in pipeline.stages() {
            if let Some(stage_id) = stage.stage_id() {
                if Id::from(stage_id).raw() == raw_id {
                    return Some(stage_id);
                }
            }
        }
    }

    None
}

fn stage_candidates<S>(pipeline: &Pipeline<S>) -> Vec<String>
where
    S: StageMeta,
{
    let mut names = Vec::new();
    for (index, stage) in pipeline.stages().iter().enumerate() {
        if let Some(name) = stage
            .stage_name()
            .and_then(|symbol| pipeline.resolve(symbol).map(str::to_string))
        {
            names.push(name);
            continue;
        }
        let raw_id = stage
            .stage_id()
            .map(|id| Id::from(id).raw())
            .unwrap_or(index);
        names.push(raw_id.to_string());
    }
    names.sort();
    names.dedup();
    names
}

fn best_stage_suggestion(stage_symbol: &str, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .min_by_key(|c| levenshtein(stage_symbol, c))
        .filter(|c| levenshtein(stage_symbol, c) <= 3)
        .cloned()
}

/// Ensure a repeated `stage` declaration is consistent with existing staged signature.
fn ensure_staged_signature_matches<L>(
    stage: &StageInfo<L>,
    staged_function: StagedFunction,
    header: &Header<'_, L::Type>,
) -> Result<(), FunctionParseError>
where
    L: Dialect,
{
    let staged_info = staged_function.expect_info(stage);
    if staged_info.signature() == &header.signature {
        return Ok(());
    }
    Err(FunctionParseError::new(
        FunctionParseErrorKind::EmitFailed,
        Some(header.span),
        "stage declaration signature does not match existing staged function",
    ))
}

fn stage_dialect_mismatch_error(
    stage_symbol: &str,
    span: Option<SimpleSpan>,
) -> FunctionParseError {
    FunctionParseError::new(
        FunctionParseErrorKind::EmitFailed,
        span,
        format!(
            "stage '@{stage_symbol}' has no registered parser dialect in this compile-stage container"
        ),
    )
}

fn stage_dispatch_miss_error(
    miss: StageDispatchMiss,
    stage_symbol: &str,
    span: Option<SimpleSpan>,
) -> FunctionParseError {
    match miss {
        StageDispatchMiss::MissingDialect => stage_dialect_mismatch_error(stage_symbol, span),
        StageDispatchMiss::MissingStage => FunctionParseError::new(
            FunctionParseErrorKind::EmitFailed,
            span,
            format!("stage '@{stage_symbol}' does not exist in the pipeline"),
        ),
    }
}

fn stage_dispatch_error(
    error: StageDispatchRequiredError<FunctionParseError>,
    stage_symbol: &str,
    span: Option<SimpleSpan>,
) -> FunctionParseError {
    match error {
        StageDispatchRequiredError::Action(error) => error,
        StageDispatchRequiredError::Miss(miss) => {
            stage_dispatch_miss_error(miss, stage_symbol, span)
        }
    }
}

/// Build a standardized error for `specialize` declarations without a matching stage header.
fn missing_stage_declaration_error<L>(
    header: &Header<'_, L>,
    span: Option<SimpleSpan>,
) -> FunctionParseError {
    FunctionParseError::new(
        FunctionParseErrorKind::MissingStageDeclaration,
        span.or(Some(header.span)),
        format!(
            "specialize declaration for stage '@{}' and function '@{}' has no matching stage declaration",
            header.stage.name, header.function.name
        ),
    )
}
