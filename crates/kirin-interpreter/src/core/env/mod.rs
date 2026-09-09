//! Environments: the storage container ([`EnvStore`]) and the engine capability that
//! operates on it ([`Env`]).

mod services;
mod store;

#[cfg(test)]
mod tests;

pub use services::{Env, SSABinding};
pub use store::{EnvIndex, EnvStore};
