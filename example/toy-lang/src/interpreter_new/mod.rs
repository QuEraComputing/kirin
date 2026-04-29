mod completion;
mod constprop;
mod error;
mod frame;
mod language;
mod run;

pub use completion::ToyCompletion;
pub use constprop::ConstProp;
pub use error::ToyError;
pub use frame::ToyFrame;
pub use run::{
    analyze_lowered_constprop, analyze_source_constprop, run_lowered_i64, run_source_i64,
};

#[cfg(test)]
mod tests;
