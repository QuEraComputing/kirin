mod completion;
mod constprop;
mod error;
mod fixpoint;
mod frame;
mod language;
mod run;

pub use completion::ToyCompletion;
pub use constprop::ConstProp;
pub use error::ToyError;
pub use fixpoint::{analyze_lowered_constprop_fixpoint, analyze_source_constprop_fixpoint};
pub use frame::ToyFrame;
pub use run::{run_lowered_i64, run_source_i64};

#[cfg(test)]
pub(crate) use fixpoint::{
    analyze_lowered_constprop_backward_dependencies, analyze_lowered_constprop_forward_dependencies,
};
#[cfg(test)]
pub use run::{analyze_lowered_constprop, analyze_source_constprop};

#[cfg(test)]
mod tests;
