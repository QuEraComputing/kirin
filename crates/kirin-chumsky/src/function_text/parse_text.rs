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
use std::collections::HashSet;

use rustc_hash::FxHashMap;

use chumsky::span::SimpleSpan;
use kirin_ir::{
    CompileStage, Dialect, Function, GetInfo, GlobalSymbol, Id, Pipeline, Signature, StageInfo,
    StageMeta, StagedFunction,
};
use kirin_lexer::Token;
use strsim::levenshtein;

use crate::{EmitContext, HasParser, ParseEmit};

use super::dispatch::ParseDispatch;
use super::error::{DiagnosticError, FunctionParseError, FunctionParseErrorKind};
use super::syntax::{Declaration, Header, RichError, parse_one_declaration, tokenize};
use crate::ast::SymbolName;

/// Parse function text into a pipeline using stage-driven dialect dispatch.
pub trait ParsePipelineText {
    fn parse(&mut self, src: &str) -> Result<Vec<Function>, FunctionParseError>;
}

// ---------------------------------------------------------------------------
// Internal types used by both the pipeline impl and the concrete helpers.
// Made pub(crate) so the dispatch module and derive-generated code can see them.
// ---------------------------------------------------------------------------

/// Composite key for staged-function lookup: `(stage, function)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StagedKey {
    /// The compile stage this function belongs to.
    pub stage: CompileStage,
    /// The abstract function handle.
    pub function: Function,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeclKeyword {
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
pub(super) struct FirstPassOutcome {
    pub(super) keyword: DeclKeyword,
    pub(super) next_index: usize,
}

/// Result of a first-pass dispatch for a single declaration.
///
/// Returned by [`first_pass_concrete`] and consumed by the pipeline parsing loop.
#[derive(Clone, Copy, Debug)]
pub struct FirstPassDispatchResult {
    pub(super) outcome: FirstPassOutcome,
    pub(super) link: Option<(Function, StagedFunction)>,
}

/// Shared mutable state threaded through both parse passes.
///
/// This type is opaque to external callers; it is only constructed inside the
/// pipeline parsing loop and passed through context structs.
pub struct ParseState {
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

    /// Record a function as touched during parsing.
    pub fn record(&mut self, function: Function) {
        if self.touched_function_set.insert(function) {
            self.touched_functions.push(function);
        }
    }
}

// ---------------------------------------------------------------------------
// Context types for concrete dispatch helpers
// ---------------------------------------------------------------------------

/// Bundled state for first-pass dispatch. Created by the pipeline impl and
/// passed to [`ParseDispatch::dispatch_first_pass`] or [`first_pass_concrete`].
pub struct FirstPassCtx<'t> {
    pub tokens: &'t [(Token<'t>, SimpleSpan)],
    pub start_index: usize,
    pub function: Option<Function>,
    pub function_symbol: Option<GlobalSymbol>,
    pub staged_lookup: &'t mut FxHashMap<StagedKey, StagedFunction>,
    pub state: &'t mut ParseState,
}

/// Bundled state for second-pass dispatch. Created by the pipeline impl and
/// passed to [`ParseDispatch::dispatch_second_pass`] or [`second_pass_concrete`].
pub struct SecondPassCtx<'t> {
    pub tokens: &'t [(Token<'t>, SimpleSpan)],
    pub start_index: usize,
    pub src: &'t str,
    pub function_lookup: &'t mut FxHashMap<String, Function>,
    pub staged_lookup: &'t mut FxHashMap<StagedKey, StagedFunction>,
    pub state: &'t mut ParseState,
    /// The function's GlobalSymbol (for auto-creating staged functions).
    pub function_symbol: GlobalSymbol,
    /// The function name (from parse_declaration_head, always available).
    pub function_name: &'t str,
    /// Link info for the pipeline to connect function ↔ staged function.
    /// Set by second_pass_concrete after apply_specialize_declaration.
    pub link: Option<(Function, StagedFunction)>,
}

// ---------------------------------------------------------------------------
// Concrete (monomorphic) helpers — called by ParseDispatch impls
// ---------------------------------------------------------------------------

/// First-pass concrete helper for a single dialect `L`.
///
/// The lifetime `'t` is the lifetime of the token slice, NOT an HRTB — this is
/// called with a concrete lifetime from within the pipeline `parse` method.
pub fn first_pass_concrete<'t, L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    ctx: &mut FirstPassCtx<'t>,
) -> Result<FirstPassDispatchResult, FunctionParseError>
where
    L: Dialect + HasParser<'t>,
    L::Type: kirin_ir::Placeholder + HasParser<'t, Output = L::Type>,
{
    let (declaration, consumed_span) = parse_one_declaration::<L>(&ctx.tokens[ctx.start_index..])
        .map_err(parse_error_from_chumsky)?;
    let next_index = advance_to_next_declaration(ctx.tokens, ctx.start_index, consumed_span);

    match declaration {
        Declaration::Stage(header) => {
            let function = ctx
                .function
                .expect("stage declaration should have a resolved function");
            let function_symbol = ctx
                .function_symbol
                .expect("stage declaration should have a function symbol");
            let staged_function = apply_stage_declaration::<L>(
                stage,
                stage_id,
                function,
                function_symbol,
                &header,
                ctx.staged_lookup,
                ctx.state,
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

/// Second-pass concrete helper for a single dialect `L`.
///
/// Called with a concrete lifetime from within the pipeline `parse` method.
pub fn second_pass_concrete<'t, L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    ctx: &mut SecondPassCtx<'t>,
) -> Result<usize, FunctionParseError>
where
    L: Dialect + ParseEmit<L> + HasParser<'t>,
    L::Type: kirin_ir::Placeholder + HasParser<'t, Output = L::Type>,
{
    let (declaration, consumed_span) = parse_one_declaration::<L>(&ctx.tokens[ctx.start_index..])
        .map_err(parse_error_from_chumsky)?;
    let next_index = advance_to_next_declaration(ctx.tokens, ctx.start_index, consumed_span);

    let Declaration::Specialize {
        stage: _stage_sym,
        function: _,
        signature,
        body_span,
        span,
    } = declaration
    else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::InvalidHeader,
            Some(consumed_span),
            "expected specialize declaration",
        ));
    };

    let body_text = &ctx.src[body_span.start..body_span.end];

    // Use the function name from parse_declaration_head (always available),
    // not from the chumsky Declaration (empty for dialect-controlled format).
    let function_name = SymbolName {
        name: ctx.function_name,
        span,
    };

    let (function, staged_function) = apply_specialize_declaration::<L>(
        stage,
        stage_id,
        &function_name,
        ctx.function_symbol,
        signature.as_ref(),
        body_text,
        span,
        &mut *ctx.function_lookup,
        &mut *ctx.staged_lookup,
        ctx.state,
    )?;

    // Return link info for the pipeline to connect function ↔ staged function
    ctx.link = Some((function, staged_function));

    Ok(next_index)
}

// ---------------------------------------------------------------------------
// Pipeline impl using ParseDispatch
// ---------------------------------------------------------------------------

impl<S> ParsePipelineText for Pipeline<S>
where
    S: StageMeta + ParseDispatch,
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
                (Some(function), Some(fn_symbol(self, function)))
            } else {
                (None, None)
            };

            let mut ctx = FirstPassCtx {
                tokens: &tokens,
                start_index: index,
                function,
                function_symbol,
                staged_lookup: &mut staged_lookup,
                state: &mut state,
            };

            let stage = self
                .stage_mut(stage_id)
                .ok_or_else(|| stage_missing_error(head.stage.name, Some(head.stage.span)))?;
            let dispatch = stage
                .dispatch_first_pass(stage_id, &mut ctx)
                .and_then(|opt| {
                    opt.ok_or_else(|| {
                        stage_dialect_mismatch_error(head.stage.name, Some(head.stage.span))
                    })
                })?;

            let outcome = dispatch.outcome;
            if let Some((function, staged_function)) = dispatch.link {
                self.link(function, stage_id, staged_function)
                    .expect("link should succeed for valid function");
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
            // Pre-create the abstract function (needed for auto-create staged fn path)
            let head = parse_declaration_head(&tokens, start_index)?;
            let function =
                get_or_create_function_by_name(self, &mut function_lookup, head.function.name);
            let function_symbol = fn_symbol(self, function);

            let mut ctx = SecondPassCtx {
                tokens: &tokens,
                start_index,
                src,
                function_lookup: &mut function_lookup,
                staged_lookup: &mut staged_lookup,
                state: &mut state,
                function_symbol,
                function_name: head.function.name,
                link: None,
            };
            let stage = self
                .stage_mut(stage_id)
                .ok_or_else(|| stage_missing_error(stage_symbol.name, Some(stage_symbol.span)))?;
            let next_index = stage
                .dispatch_second_pass(stage_id, &mut ctx)
                .and_then(|opt| {
                    opt.ok_or_else(|| {
                        stage_dialect_mismatch_error(stage_symbol.name, Some(stage_symbol.span))
                    })
                })?;
            // Link the function ↔ staged function if created in this pass
            if let Some((function, staged_function)) = ctx.link {
                self.link(function, stage_id, staged_function)
                    .expect("link should succeed for auto-created function");
            }

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

// ---------------------------------------------------------------------------
// Internal helpers (unchanged from original)
// ---------------------------------------------------------------------------

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
    staged_lookup: &mut FxHashMap<StagedKey, StagedFunction>,
    state: &mut ParseState,
) -> Result<Option<StagedFunction>, FunctionParseError>
where
    L: Dialect,
    L::Type: kirin_ir::Placeholder,
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
        .with_builder(|builder| {
            builder
                .staged_function()
                .name(function_symbol)
                .signature(header.signature.clone())
                .new()
        })
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

fn apply_specialize_declaration<L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    function_name: &SymbolName<'_>,
    function_symbol: GlobalSymbol,
    framework_signature: Option<&Signature<L::Type>>,
    body_text: &str,
    span: SimpleSpan,
    function_lookup: &mut FxHashMap<String, Function>,
    staged_lookup: &mut FxHashMap<StagedKey, StagedFunction>,
    state: &mut ParseState,
) -> Result<(Function, StagedFunction), FunctionParseError>
where
    L: Dialect + ParseEmit<L>,
    L::Type: kirin_ir::Placeholder,
{
    // Parse and emit the body first — we need it to extract signature if needed
    let body_statement = stage
        .with_builder(|builder| {
            let mut emit_ctx = EmitContext::new(builder);
            L::parse_and_emit(body_text, &mut emit_ctx).map_err(|err| {
                let (kind, message) = match &err {
                    crate::ChumskyError::Parse(errs) => (
                        FunctionParseErrorKind::BodyParseFailed,
                        errs.iter()
                            .map(|e| e.to_string())
                            .collect::<Vec<_>>()
                            .join("; "),
                    ),
                    crate::ChumskyError::Emit(e) => {
                        (FunctionParseErrorKind::EmitFailed, e.to_string())
                    }
                };
                FunctionParseError::new(kind, Some(span), message)
            })
        })
        .map_err(|err| {
            FunctionParseError::new(
                FunctionParseErrorKind::EmitFailed,
                Some(span),
                err.to_string(),
            )
        })?;

    // Determine signature: framework-parsed or dialect-extracted
    let signature = if let Some(sig) = framework_signature {
        sig.clone()
    } else if let Some(sig) = L::extract_signature(body_statement, stage) {
        sig
    } else {
        return Err(FunctionParseError::new(
            FunctionParseErrorKind::EmitFailed,
            Some(span),
            format!(
                "no function signature available for '{}'. Either use `stage` declaration \
                 with explicit signature, or implement `extract_signature` on ParseEmit.",
                function_name.name
            ),
        ));
    };

    // Resolve or auto-create the staged function
    let (function, staged_function) = resolve_or_create_specialize_target::<L>(
        stage,
        stage_id,
        function_name,
        function_symbol,
        &signature,
        span,
        function_lookup,
        staged_lookup,
    )?;

    // Construct the specialization
    stage
        .with_builder(|builder| {
            builder
                .specialize()
                .staged_func(staged_function)
                .signature(signature.clone())
                .body(body_statement)
                .new()
                .map_err(|err| {
                    FunctionParseError::new(
                        FunctionParseErrorKind::EmitFailed,
                        Some(span),
                        err.to_string(),
                    )
                })
        })
        .map_err(|err| {
            FunctionParseError::new(
                FunctionParseErrorKind::EmitFailed,
                Some(span),
                err.to_string(),
            )
        })?;

    state.record(function);
    Ok((function, staged_function))
}

fn resolve_or_create_specialize_target<L>(
    stage: &mut StageInfo<L>,
    stage_id: CompileStage,
    function_name: &SymbolName<'_>,
    function_symbol: GlobalSymbol,
    signature: &Signature<L::Type>,
    span: SimpleSpan,
    function_lookup: &mut FxHashMap<String, Function>,
    staged_lookup: &mut FxHashMap<StagedKey, StagedFunction>,
) -> Result<(Function, StagedFunction), FunctionParseError>
where
    L: Dialect,
    L::Type: kirin_ir::Placeholder,
{
    // Look up existing function
    let function = function_lookup
        .get(function_name.name)
        .copied()
        .ok_or_else(|| {
            FunctionParseError::new(
                FunctionParseErrorKind::MissingStageDeclaration,
                Some(span),
                format!(
                    "specialize declaration for function '@{}' has no matching function",
                    function_name.name
                ),
            )
        })?;

    let key = StagedKey {
        stage: stage_id,
        function,
    };

    // Return existing staged function if present
    if let Some(staged_function) = staged_lookup.get(&key).copied() {
        return Ok((function, staged_function));
    }

    // Auto-create staged function from extracted signature.
    // This supports `specialize` without a preceding `stage` declaration.
    let staged_function = stage
        .with_builder(|builder| {
            builder
                .staged_function()
                .name(function_symbol)
                .signature(signature.clone())
                .new()
        })
        .map_err(|err| {
            FunctionParseError::new(
                FunctionParseErrorKind::EmitFailed,
                Some(span),
                format!("failed to auto-create staged function: {}", err),
            )
        })?;

    staged_lookup.insert(key, staged_function);
    Ok((function, staged_function))
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

fn parse_error_from_chumsky(errors: Vec<RichError<'_>>) -> FunctionParseError {
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
fn collect_function_lookup<S>(pipeline: &Pipeline<S>) -> FxHashMap<String, Function> {
    let mut lookup = FxHashMap::default();
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
    function_lookup: &mut FxHashMap<String, Function>,
    name: &str,
) -> Function {
    if let Some(existing) = function_lookup.get(name).copied() {
        return existing;
    }
    let function = pipeline
        .function()
        .name(name.to_string())
        .new()
        .expect("failed to create function in pipeline");
    function_lookup.insert(name.to_string(), function);
    function
}

fn fn_symbol<S>(pipeline: &Pipeline<S>, function: Function) -> GlobalSymbol {
    pipeline
        .function_info(function)
        .and_then(|info| info.name())
        .expect("stage declarations should always use named functions")
}

/// Build a `(stage, function) -> staged function` lookup from existing pipeline state.
fn collect_staged_lookup<S>(pipeline: &Pipeline<S>) -> FxHashMap<StagedKey, StagedFunction> {
    let mut lookup = FxHashMap::default();
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

    if let Some(suggestion) = best_stage_suggestion(stage_symbol, &candidates)
        && !output.contains(&suggestion)
    {
        output.push_str(&format!(", did you mean '@{suggestion}'?"));
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
            && name == stage_symbol
            && let Some(stage_id) = stage.stage_id()
        {
            return Some(stage_id);
        }
    }

    if let Ok(raw_id) = stage_symbol.parse::<usize>() {
        for stage in pipeline.stages() {
            if let Some(stage_id) = stage.stage_id()
                && Id::from(stage_id).raw() == raw_id
            {
                return Some(stage_id);
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

fn stage_missing_error(stage_symbol: &str, span: Option<SimpleSpan>) -> FunctionParseError {
    FunctionParseError::new(
        FunctionParseErrorKind::EmitFailed,
        span,
        format!("stage '@{stage_symbol}' does not exist in the pipeline"),
    )
}

