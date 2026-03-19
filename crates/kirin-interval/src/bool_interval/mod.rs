mod domain;
#[cfg(feature = "interpreter")]
mod interpreter_impl;
mod lattice_impl;
#[cfg(test)]
mod tests;

pub use domain::BoolInterval;
