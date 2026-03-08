mod semantics;
#[allow(clippy::module_inception)]
mod signature;

#[cfg(test)]
mod tests;

pub use semantics::{ExactSemantics, LatticeSemantics, SignatureCmp, SignatureSemantics};
pub use signature::Signature;
