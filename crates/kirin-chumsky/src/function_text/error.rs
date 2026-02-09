use chumsky::span::SimpleSpan;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Error categories for function-text parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionParseErrorKind {
    InvalidHeader,
    UnknownStage,
    InconsistentFunctionName,
    MissingStageDeclaration,
    BodyParseFailed,
    EmitFailed,
}

impl Display for FunctionParseErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FunctionParseErrorKind::InvalidHeader => write!(f, "invalid function header"),
            FunctionParseErrorKind::UnknownStage => write!(f, "unknown stage"),
            FunctionParseErrorKind::InconsistentFunctionName => {
                write!(f, "inconsistent function name")
            }
            FunctionParseErrorKind::MissingStageDeclaration => {
                write!(f, "missing stage declaration")
            }
            FunctionParseErrorKind::BodyParseFailed => write!(f, "function body parse failed"),
            FunctionParseErrorKind::EmitFailed => write!(f, "IR emission failed"),
        }
    }
}

/// A domain error for function-text parse failures.
#[derive(Debug)]
pub struct FunctionParseError {
    pub kind: FunctionParseErrorKind,
    pub span: Option<SimpleSpan>,
    pub message: String,
    pub source: Option<Box<dyn Error + Send + Sync>>,
}

impl FunctionParseError {
    pub(crate) fn new(
        kind: FunctionParseErrorKind,
        span: Option<SimpleSpan>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_source(mut self, source: impl Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }
}

impl Display for FunctionParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.span {
            Some(span) => write!(
                f,
                "{} at {}..{}: {}",
                self.kind, span.start, span.end, self.message
            ),
            None => write!(f, "{}: {}", self.kind, self.message),
        }
    }
}

impl Error for FunctionParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.as_ref() as &(dyn Error + 'static))
    }
}

#[derive(Debug)]
pub(crate) struct DiagnosticError {
    diagnostics: Vec<String>,
}

impl DiagnosticError {
    pub(crate) fn new(diagnostics: Vec<String>) -> Self {
        Self { diagnostics }
    }
}

impl Display for DiagnosticError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostics.join("\n"))
    }
}

impl Error for DiagnosticError {}
