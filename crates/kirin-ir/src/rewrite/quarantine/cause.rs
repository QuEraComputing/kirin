//! Why a rewrite pass failed.

use std::any::Any;
use std::fmt;

use crate::VerifyError;

/// Why a pass failed.
#[derive(Debug)]
pub enum QuarantineCause {
    /// The pass closure returned an error
    Pass(Box<dyn std::error::Error + Send + Sync>),
    /// The pass finished but the IR was left in an invalid state
    Verify(VerifyError),
    /// The pass panicked. Holds the panic message.
    Panic(String),
}

impl QuarantineCause {
    /// Build a [`QuarantineCause::Panic`] from a [`catch_unwind`](std::panic::catch_unwind) payload.
    pub(crate) fn from_panic(panic: Box<dyn Any + Send>) -> Self {
        let message = panic
            // panic argument may be `&'static str`
            .downcast_ref::<&'static str>()
            .map(|message| (*message).to_string())
            // panic argument may be `String`
            .or_else(|| panic.downcast_ref::<String>().cloned())
            // anything else defaults
            .unwrap_or_else(|| "panicked with a non-string payload".to_string());

        QuarantineCause::Panic(message)
    }
}

impl fmt::Display for QuarantineCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuarantineCause::Pass(error) => write!(f, "the pass returned an error: {error}"),
            QuarantineCause::Verify(error) => write!(f, "{error}"),
            QuarantineCause::Panic(message) => write!(f, "the pass panicked: {message}"),
        }
    }
}
