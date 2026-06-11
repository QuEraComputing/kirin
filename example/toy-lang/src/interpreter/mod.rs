mod completion;
mod concrete;
mod cross_stage;
mod error;
mod fixpoint;
mod frame;
mod profile;
mod run;

#[cfg(test)]
mod tests;

pub use completion::ToyCompletion;
#[allow(unused_imports)]
pub use concrete::ToyConcreteInterpreter;
pub use concrete::run_i64;
pub use error::ToyError;
pub use fixpoint::{analyze_lowered_constprop_fixpoint, analyze_source_constprop_fixpoint};
pub use frame::{ToyFrame, ToyStageFrame};
pub use profile::{ToyLoweredConcrete, ToySourceConcrete};
pub use run::{run_lowered_i64, run_source_i64};

type ConstProp = kirin_constprop::ConstPropValue;
