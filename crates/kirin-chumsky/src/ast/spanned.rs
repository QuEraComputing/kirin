use chumsky::span::SimpleSpan;
use kirin_ir::Dialect;

use crate::traits::{EmitContext, EmitError, EmitIR};

/// A value with an associated span.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: SimpleSpan,
}

impl<T: Copy> Copy for Spanned<T> {}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl<T> Spanned<T> {
    /// Creates a new spanned value.
    pub fn new(value: T, span: SimpleSpan) -> Self {
        Self { value, span }
    }

    /// Maps the inner value using the provided function.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            value: f(self.value),
            span: self.span,
        }
    }
}

/// Implementation of EmitIR for Spanned values.
///
/// This simply delegates to the inner value's EmitIR implementation.
impl<T, IR> EmitIR<IR> for Spanned<T>
where
    IR: Dialect,
    T: EmitIR<IR>,
{
    type Output = T::Output;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        self.value.emit(ctx)
    }
}
