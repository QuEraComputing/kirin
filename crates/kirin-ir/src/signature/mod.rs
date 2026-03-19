mod definition;
mod semantics;

#[cfg(test)]
mod tests;

pub use definition::Signature;
pub use semantics::{ExactSemantics, LatticeSemantics, SignatureCmp, SignatureSemantics};
