use kirin_arith::ArithType;
use kirin_ir::{Dialect, SSAValue};

/// Test language for punctuation at optional format section boundaries.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Dialect,
    kirin_chumsky::HasParser,
    kirin_derive_chumsky::PrettyPrint,
)]
#[kirin(type = ArithType, crate = kirin_ir)]
#[chumsky(crate = kirin_chumsky)]
#[pretty(crate = kirin_prettyless)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OptionalSpacingLanguage {
    #[chumsky(format = "$bare {amplitude}[, {duration}] -> {result_type}")]
    Bare {
        amplitude: SSAValue,
        duration: Vec<SSAValue>,
        result_type: ArithType,
    },
    #[chumsky(format = "$grouped ({amplitude})[, {duration}] -> {result_type}")]
    Grouped {
        amplitude: SSAValue,
        duration: Vec<SSAValue>,
        result_type: ArithType,
    },
    #[chumsky(format = "$close_before }[} {values}]")]
    CloseBefore { values: Vec<SSAValue> },
    #[chumsky(format = "$close_after }[ {values}] }")]
    CloseAfter { values: Vec<SSAValue> },
    #[chumsky(format = "$close_chain }[ {first}][, {second}] }")]
    CloseChain {
        first: Vec<SSAValue>,
        second: Vec<SSAValue>,
    },
}
