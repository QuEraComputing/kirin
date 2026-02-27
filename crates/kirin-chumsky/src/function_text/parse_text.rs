use std::collections::{HashMap, HashSet};

use chumsky::span::SimpleSpan;
use kirin_ir::{
    CompileStage, Dialect, Function, GetInfo, HasStageInfo, Id, Pipeline, StageMeta,
    StagedFunction, Statement,
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
}

#[derive(Clone, Copy, Debug)]
struct FirstPassOutcome {
    keyword: DeclKeyword,
    next_index: usize,
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

trait StageDialects<S: StageMeta> {
    fn first_pass<'src>(
        pipeline: &mut Pipeline<S>,
        tokens: &[(Token<'src>, SimpleSpan)],
        start_index: usize,
        stage_id: CompileStage,
        stage_symbol: &SymbolName<'src>,
        staged_lookup: &mut HashMap<StagedKey, StagedFunction>,
        state: &mut ParseState,
    ) -> Result<FirstPassOutcome, FunctionParseError>;

    fn second_pass_specialize<'src>(
        pipeline: &mut Pipeline<S>,
        tokens: &[(Token<'src>, SimpleSpan)],
        start_index: usize,
        stage_id: CompileStage,
        stage_symbol: &SymbolName<'src>,
        staged_lookup: &HashMap<StagedKey, StagedFunction>,
        state: &mut ParseState,
    ) -> Result<usize, FunctionParseError>;
}

impl<S> StageDialects<S> for ()
where
    S: StageMeta,
{
    fn first_pass<'src>(
        _pipeline: &mut Pipeline<S>,
        _tokens: &[(Token<'src>, SimpleSpan)],
        _start_index: usize,
        _stage_id: CompileStage,
        stage_symbol: &SymbolName<'src>,
        _staged_lookup: &mut HashMap<StagedKey, StagedFunction>,
        _state: &mut ParseState,
    ) -> Result<FirstPassOutcome, FunctionParseError> {
        Err(stage_dialect_mismatch_error(
            stage_symbol.name,
            Some(stage_symbol.span),
        ))
    }

    fn second_pass_specialize<'src>(
        _pipeline: &mut Pipeline<S>,
        _tokens: &[(Token<'src>, SimpleSpan)],
        _start_index: usize,
        _stage_id: CompileStage,
        stage_symbol: &SymbolName<'src>,
        _staged_lookup: &HashMap<StagedKey, StagedFunction>,
        _state: &mut ParseState,
    ) -> Result<usize, FunctionParseError> {
        Err(stage_dialect_mismatch_error(
            stage_symbol.name,
            Some(stage_symbol.span),
        ))
    }
}

impl<S, L, Tail> StageDialects<S> for (L, Tail)
where
    S: StageMeta + HasStageInfo<L>,
    Tail: StageDialects<S>,
    L: Dialect,
    for<'src> L: HasParser<'src, 'src>,
    for<'src> L::Type: HasParser<'src, 'src, Output = L::Type>,
    for<'src> <L as HasParser<'src, 'src>>::Output: EmitIR<L, Output = Statement>,
{
    fn first_pass<'src>(
        pipeline: &mut Pipeline<S>,
        tokens: &[(Token<'src>, SimpleSpan)],
        start_index: usize,
        stage_id: CompileStage,
        stage_symbol: &SymbolName<'src>,
        staged_lookup: &mut HashMap<StagedKey, StagedFunction>,
        state: &mut ParseState,
    ) -> Result<FirstPassOutcome, FunctionParseError> {
        if !stage_supports_dialect::<L, S>(pipeline, stage_id) {
            return Tail::first_pass(
                pipeline,
                tokens,
                start_index,
                stage_id,
                stage_symbol,
                staged_lookup,
                state,
            );
        }

        let (declaration, consumed_span) =
            parse_one_declaration::<L>(&tokens[start_index..]).map_err(parse_error_from_chumsky)?;
        let next_index = advance_to_next_declaration(tokens, start_index, consumed_span);

        let keyword = match declaration {
            Declaration::Stage(header) => {
                apply_stage_declaration::<L, S>(pipeline, stage_id, &header, staged_lookup, state)?;
                DeclKeyword::Stage
            }
            Declaration::Specialize { .. } => DeclKeyword::Specialize,
        };

        Ok(FirstPassOutcome {
            keyword,
            next_index,
        })
    }

    fn second_pass_specialize<'src>(
        pipeline: &mut Pipeline<S>,
        tokens: &[(Token<'src>, SimpleSpan)],
        start_index: usize,
        stage_id: CompileStage,
        stage_symbol: &SymbolName<'src>,
        staged_lookup: &HashMap<StagedKey, StagedFunction>,
        state: &mut ParseState,
    ) -> Result<usize, FunctionParseError> {
        if !stage_supports_dialect::<L, S>(pipeline, stage_id) {
            return Tail::second_pass_specialize(
                pipeline,
                tokens,
                start_index,
                stage_id,
                stage_symbol,
                staged_lookup,
                state,
            );
        }

        let (declaration, consumed_span) =
            parse_one_declaration::<L>(&tokens[start_index..]).map_err(parse_error_from_chumsky)?;
        let next_index = advance_to_next_declaration(tokens, start_index, consumed_span);

        let Declaration::Specialize { header, body, span } = declaration else {
            return Err(FunctionParseError::new(
                FunctionParseErrorKind::InvalidHeader,
                Some(consumed_span),
                "expected specialize declaration",
            ));
        };

        apply_specialize_declaration::<L, S>(
            pipeline,
            stage_id,
            &header,
            &body,
            span,
            staged_lookup,
            state,
        )?;

        Ok(next_index)
    }
}

impl<S> ParsePipelineText for Pipeline<S>
where
    S: StageMeta,
    S::Languages: StageDialects<S>,
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
        let mut state = ParseState::new();
        let mut pending_specializations: Vec<(usize, CompileStage, SymbolName<'_>)> = Vec::new();

        let mut index = 0;
        while index < tokens.len() {
            let head = parse_declaration_head(&tokens, index)?;
            let stage_id = resolve_or_create_stage_symbol(self, &head.stage)?;

            let outcome = <S::Languages as StageDialects<S>>::first_pass(
                self,
                &tokens,
                index,
                stage_id,
                &head.stage,
                &mut staged_lookup,
                &mut state,
            )?;

            if outcome.keyword != head.keyword {
                return Err(FunctionParseError::new(
                    FunctionParseErrorKind::InvalidHeader,
                    Some(head.stage.span),
                    "declaration keyword mismatch while parsing",
                ));
            }
            if outcome.next_index <= index {
                return Err(FunctionParseError::new(
                    FunctionParseErrorKind::InvalidHeader,
                    Some(head.stage.span),
                    "failed to advance while parsing declaration",
                ));
            }

            if matches!(outcome.keyword, DeclKeyword::Specialize) {
                pending_specializations.push((index, stage_id, head.stage));
            }

            index = outcome.next_index;
        }

        for (start_index, stage_id, stage_symbol) in pending_specializations {
            let next_index = <S::Languages as StageDialects<S>>::second_pass_specialize(
                self,
                &tokens,
                start_index,
                stage_id,
                &stage_symbol,
                &staged_lookup,
                &mut state,
            )?;

            if next_index <= start_index {
                return Err(FunctionParseError::new(
                    FunctionParseErrorKind::InvalidHeader,
                    Some(stage_symbol.span),
                    "failed to advance while parsing specialize declaration",
                ));
            }
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

    Ok(DeclarationHead {
        keyword,
        stage: SymbolName {
            name: stage_name,
            span: *stage_span,
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

fn stage_supports_dialect<L, S>(pipeline: &Pipeline<S>, stage_id: CompileStage) -> bool
where
    L: Dialect,
    S: HasStageInfo<L>,
{
    pipeline
        .stage(stage_id)
        .and_then(|stage| <S as HasStageInfo<L>>::try_stage_info(stage))
        .is_some()
}

fn apply_stage_declaration<'src, L, S>(
    pipeline: &mut Pipeline<S>,
    stage_id: CompileStage,
    header: &Header<'src, L::Type>,
    staged_lookup: &mut HashMap<StagedKey, StagedFunction>,
    state: &mut ParseState,
) -> Result<(), FunctionParseError>
where
    L: Dialect,
    S: HasStageInfo<L>,
{
    let function = get_or_create_function_by_name(pipeline, header.function.name);
    state.record(function);

    let key = StagedKey {
        stage: stage_id,
        function,
    };
    if let Some(existing) = staged_lookup.get(&key).copied() {
        ensure_staged_signature_matches::<L, S>(pipeline, stage_id, existing, header)?;
        return Ok(());
    }

    let staged_function = pipeline
        .staged_function()
        .func(function)
        .stage(stage_id)
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

    Ok(())
}

fn apply_specialize_declaration<'src, L, S>(
    pipeline: &mut Pipeline<S>,
    stage_id: CompileStage,
    header: &Header<'src, L::Type>,
    body: &<L as HasParser<'src, 'src>>::Output,
    span: SimpleSpan,
    staged_lookup: &HashMap<StagedKey, StagedFunction>,
    state: &mut ParseState,
) -> Result<(), FunctionParseError>
where
    L: Dialect + HasParser<'src, 'src>,
    L::Type: HasParser<'src, 'src, Output = L::Type>,
    <L as HasParser<'src, 'src>>::Output: EmitIR<L, Output = Statement>,
    S: HasStageInfo<L>,
{
    let Some(function) = find_function_by_name(pipeline, header.function.name) else {
        return Err(missing_stage_declaration_error(header, Some(span)));
    };
    let key = StagedKey {
        stage: stage_id,
        function,
    };
    let Some(staged_function) = staged_lookup.get(&key).copied() else {
        return Err(missing_stage_declaration_error(header, Some(span)));
    };

    let stage_entry = pipeline
        .stage_mut(stage_id)
        .expect("resolved stage should exist");
    let Some(stage) = <S as HasStageInfo<L>>::try_stage_info_mut(stage_entry) else {
        return Err(stage_dialect_mismatch_error(
            header.stage.name,
            Some(header.stage.span),
        ));
    };

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

/// Find an existing abstract function by its resolved global symbol name.
fn find_function_by_name<S>(pipeline: &Pipeline<S>, name: &str) -> Option<Function> {
    for info in pipeline.function_arena().iter() {
        let function = Function::from(info.clone().unwrap());
        if let Some(symbol) = info.name() {
            if pipeline
                .resolve(symbol)
                .is_some_and(|resolved| resolved == name)
            {
                return Some(function);
            }
        }
    }
    None
}

/// Resolve an abstract function by name, creating it if it does not exist.
fn get_or_create_function_by_name<S>(pipeline: &mut Pipeline<S>, name: &str) -> Function {
    pipeline.intern(name.to_string());
    if let Some(existing) = find_function_by_name(pipeline, name) {
        return existing;
    }
    pipeline.function().name(name.to_string()).new()
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
fn ensure_staged_signature_matches<L, S>(
    pipeline: &Pipeline<S>,
    stage_id: CompileStage,
    staged_function: StagedFunction,
    header: &Header<'_, L::Type>,
) -> Result<(), FunctionParseError>
where
    L: Dialect,
    S: HasStageInfo<L>,
{
    let stage_entry = pipeline.stage(stage_id).expect("stage must exist");
    let stage = <S as HasStageInfo<L>>::try_stage_info(stage_entry)
        .expect("stage must contain dialect for staged signature check");
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
