use std::fmt;

use kirin_ir::{FinalizeError, Statement};

/// Why a pattern was rejected as a rewrite rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleError {
    /// The rule's name, or `None` if the statement was not a pattern.
    pub rule: Option<String>,
    pub kind: RuleErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleErrorKind {
    /// The pattern stage failed to finalize.
    Finalize(FinalizeError),
    /// The statement is not a `wildcard.pattern`.
    NotAPattern(Statement),
    /// The pattern body does not end in a `wildcard.rewrite`.
    MissingRewrite,
    /// The rewrite's root is not a statement of the pattern body.
    RootNotInBody,
    /// The root is not an op of the matched language.
    RootNotAnOp,
    /// A matched op owns blocks, CFGs, graphs, or successors, which the
    /// matcher does not compare yet.
    OpWithBodies { position: usize },
    /// An op in the pattern body is not the root or one of its producers, so
    /// nothing would match it.
    UnmatchedOp { position: usize },
    /// An operand of a matched op is neither a hole nor the result of a
    /// matched op.
    UnsupportedOperand { position: usize },
    /// A pattern-body statement that cannot appear there.
    UnexpectedStatement { position: usize },
    /// A `constraint` tests a value no `native_call` computed.
    ConstraintWithoutNativeCall { position: usize },
    /// A `replace` redirects a value that is not a result of a matched op.
    ReplaceNotMatched { position: usize },
    /// The `erase` at this position of the rewrite body does not name a
    /// matched op.
    EraseNotMatched { position: usize },
    /// A rewrite-body statement the driver cannot run yet, such as building
    /// a new op.
    UnsupportedAction { position: usize },
    /// A `native_call` in the pattern body named this has no check function
    /// attached.
    UnknownCheck(String),
    /// A `native_call` in the rewrite body named this has no action function
    /// attached.
    UnknownAction(String),
    /// A `native_call` in the pattern body named this has two check
    /// functions attached, e.g. by two `check_at`s for its statement.
    TwoChecks(String),
    /// A `native_call` in the rewrite body named this has two action
    /// functions attached.
    TwoActions(String),
    /// The rewrite body ends in a terminator that is not a walk result.
    UnexpectedTerminator,
    /// The rewrite body's `walk_result` reads a value no `native_call` of
    /// the rewrite body computed.
    WalkResultWithoutNativeCall,
}

impl RuleError {
    pub(crate) fn new(rule: &str, kind: RuleErrorKind) -> Self {
        Self {
            rule: Some(rule.to_string()),
            kind,
        }
    }
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(rule) = &self.rule {
            write!(f, "rule `{rule}`: ")?;
        }
        match &self.kind {
            RuleErrorKind::Finalize(error) => {
                write!(f, "the pattern stage did not finalize: {error:?}")
            }
            RuleErrorKind::NotAPattern(stmt) => write!(f, "{stmt:?} is not a `wildcard.pattern`"),
            RuleErrorKind::MissingRewrite => {
                write!(f, "the pattern body does not end in a `rewrite`")
            }
            RuleErrorKind::RootNotInBody => {
                write!(f, "the root is not a statement of the pattern body")
            }
            RuleErrorKind::RootNotAnOp => {
                write!(f, "the root is not an op of the matched language")
            }
            RuleErrorKind::OpWithBodies { position } => write!(
                f,
                "the matched op at position {position} owns bodies or successors, which cannot be matched yet"
            ),
            RuleErrorKind::UnmatchedOp { position } => write!(
                f,
                "the op at position {position} is neither the root nor one of its producers"
            ),
            RuleErrorKind::UnsupportedOperand { position } => write!(
                f,
                "an operand of the op at position {position} is neither a hole nor a matched result"
            ),
            RuleErrorKind::UnexpectedStatement { position } => {
                write!(
                    f,
                    "the statement at position {position} cannot appear in a pattern body"
                )
            }
            RuleErrorKind::ConstraintWithoutNativeCall { position } => write!(
                f,
                "the constraint at position {position} does not test a `native_call` result"
            ),
            RuleErrorKind::ReplaceNotMatched { position } => write!(
                f,
                "the replace at position {position} of the rewrite body redirects a value the match does not define"
            ),
            RuleErrorKind::EraseNotMatched { position } => write!(
                f,
                "the `erase` at position {position} of the rewrite body does not name a matched op"
            ),
            RuleErrorKind::UnsupportedAction { position } => write!(
                f,
                "the statement at position {position} of the rewrite body is not supported yet"
            ),
            RuleErrorKind::UnknownCheck(name) => {
                write!(f, "a call of `{name}` has no check function attached")
            }
            RuleErrorKind::UnknownAction(name) => {
                write!(f, "a call of `{name}` has no action function attached")
            }
            RuleErrorKind::TwoChecks(name) => {
                write!(f, "a call of `{name}` has two check functions attached")
            }
            RuleErrorKind::TwoActions(name) => {
                write!(f, "a call of `{name}` has two action functions attached")
            }
            RuleErrorKind::UnexpectedTerminator => write!(
                f,
                "the rewrite body ends in a terminator that is not `advance`, `skip`, `interrupt` or `walk_result`"
            ),
            RuleErrorKind::WalkResultWithoutNativeCall => write!(
                f,
                "the `walk_result` does not read a value a `native_call` of the rewrite body computed"
            ),
        }
    }
}

impl std::error::Error for RuleError {}
