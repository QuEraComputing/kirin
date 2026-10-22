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
    /// An `erase` names the root, which the driver is visiting when the rule
    /// runs. The dead-code pass removes a root left unused instead.
    ErasesRoot,
    /// A rewrite-body statement the driver cannot run yet, such as building
    /// a new op.
    UnsupportedAction { position: usize },
    /// A pattern body calls a check function that is not registered.
    UnknownCheck(String),
    /// A rewrite body calls an action function that is not registered.
    UnknownAction(String),
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
            RuleErrorKind::ErasesRoot => write!(
                f,
                "the rewrite erases its root; leave a dead root to dead-code elimination"
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
                write!(f, "no check function is registered as `{name}`")
            }
            RuleErrorKind::UnknownAction(name) => {
                write!(f, "no action function is registered as `{name}`")
            }
        }
    }
}

impl std::error::Error for RuleError {}
