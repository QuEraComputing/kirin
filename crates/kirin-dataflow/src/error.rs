//! Errors surfaced by the liveness analysis.

use kirin_ir::Statement;

/// Failures the liveness analysis can report.
///
/// The analysis refuses to silently over- or under-approximate: unmodelled
/// control flow and malformed edges are explicit errors rather than guesses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LivenessError {
    /// The function-body op did not expose a region to analyse.
    #[error("function body statement {0:?} has no region")]
    NoBody(Statement),

    /// A statement carries nested blocks/regions that the analysis does not
    /// model (e.g. an unrecognised structured-control-flow op).
    #[error("unsupported structured control flow at statement {0:?}")]
    UnsupportedStructuredControlFlow(Statement),

    /// A block terminator could not be classified as a branch or a return,
    /// or a structured body was not terminated by `yield`.
    #[error("unsupported terminator at statement {0:?}")]
    UnsupportedTerminator(Statement),

    /// A CFG edge's argument count does not match its target block's
    /// parameter count, so params cannot be mapped back to edge args.
    #[error(
        "malformed CFG edge: {edge_args} edge args but target block has {block_params} parameters"
    )]
    MalformedEdge {
        /// Number of arguments supplied on the edge.
        edge_args: usize,
        /// Number of parameters declared by the target block.
        block_params: usize,
    },

    /// No stage with the requested name exists in the pipeline.
    #[error("no stage named `{0}`")]
    MissingStage(String),

    /// No function with the requested name exists at the requested stage.
    #[error("no function named `{0}` at the requested stage")]
    MissingFunction(String),

    /// The staged function does not have a unique live specialization to
    /// analyse.
    #[error("could not resolve a unique specialization: {0}")]
    Specialization(String),
}
