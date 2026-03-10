mod dispatch;
mod error;
mod parse_text;
mod syntax;

pub use dispatch::ParseDispatch;
pub use error::{FunctionParseError, FunctionParseErrorKind};
pub use parse_text::{
    FirstPassCtx, FirstPassDispatchResult, ParsePipelineText, ParseState, SecondPassCtx, StagedKey,
    first_pass_concrete, second_pass_concrete,
};

#[cfg(test)]
mod tests;
