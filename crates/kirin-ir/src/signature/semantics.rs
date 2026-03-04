use std::marker::PhantomData;

use crate::lattice::TypeLattice;

use super::signature::Signature;

/// Result of comparing two candidate signatures for specialization dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureCmp {
    /// Left candidate is more specific than right.
    More,
    /// Left candidate is less specific than right.
    Less,
    /// Both candidates are equally specific.
    Equal,
    /// Candidates are incomparable (neither is more specific).
    Incomparable,
}

/// Trait defining specialization semantics for function signatures.
///
/// When specifying a compilation pipeline (e.g., language A -> B -> C),
/// all languages in the pipeline should use the same `SignatureSemantics`
/// so that signatures are aligned across compilation stages.
pub trait SignatureSemantics<T, C = ()> {
    /// Environment produced when a candidate is found applicable.
    /// For example, type-variable bindings or solved symbols.
    type Env;

    /// Is the candidate signature applicable to this call signature?
    /// Returns `Some(env)` if applicable, `None` otherwise.
    fn applicable(call: &Signature<T, C>, cand: &Signature<T, C>) -> Option<Self::Env>;

    /// Compare two candidates, both assumed applicable.
    /// Returns the relative specificity of `a` vs `b`.
    fn cmp_candidate(
        a: &Signature<T, C>,
        a_env: &Self::Env,
        b: &Signature<T, C>,
        b_env: &Self::Env,
    ) -> SignatureCmp;
}

/// Exact-match semantics: applicable only when params and ret are exactly equal.
///
/// No lattice structure required. Suitable for simple nominal type systems.
pub struct ExactSemantics;

impl<T: PartialEq, C: PartialEq> SignatureSemantics<T, C> for ExactSemantics {
    type Env = ();

    fn applicable(call: &Signature<T, C>, cand: &Signature<T, C>) -> Option<Self::Env> {
        if call.params.len() != cand.params.len() {
            return None;
        }
        let params_match = call
            .params
            .iter()
            .zip(cand.params.iter())
            .all(|(a, b)| a == b);
        if params_match && call.ret == cand.ret && call.constraints == cand.constraints {
            Some(())
        } else {
            None
        }
    }

    fn cmp_candidate(
        a: &Signature<T, C>,
        _a_env: &Self::Env,
        b: &Signature<T, C>,
        _b_env: &Self::Env,
    ) -> SignatureCmp {
        if a == b {
            SignatureCmp::Equal
        } else {
            SignatureCmp::Incomparable
        }
    }
}

/// Lattice-based semantics: applicable when all call params are subtypes
/// (`is_subseteq`) of the candidate params.
///
/// Requires `T: TypeLattice`. Provides subtype-based specialization dispatch
/// where more specific (smaller in the lattice) candidates are preferred.
pub struct LatticeSemantics<T: TypeLattice>(PhantomData<T>);

impl<T: TypeLattice> SignatureSemantics<T> for LatticeSemantics<T> {
    type Env = ();

    fn applicable(call: &Signature<T>, cand: &Signature<T>) -> Option<Self::Env> {
        if call.params.len() != cand.params.len() {
            return None;
        }
        if call.constraints != cand.constraints {
            return None;
        }
        // Call return type must match candidate return type if specified
        if call.ret != cand.ret {
            return None;
        }
        // Call params must be subtypes (more specific or equal) of candidate params
        let all_applicable = call
            .params
            .iter()
            .zip(cand.params.iter())
            .all(|(call_param, cand_param)| call_param.is_subseteq(cand_param));
        if all_applicable { Some(()) } else { None }
    }

    fn cmp_candidate(
        a: &Signature<T>,
        _a_env: &Self::Env,
        b: &Signature<T>,
        _b_env: &Self::Env,
    ) -> SignatureCmp {
        if a.params.len() != b.params.len() {
            return SignatureCmp::Incomparable;
        }

        let a_sub_b = a
            .params
            .iter()
            .zip(b.params.iter())
            .all(|(ap, bp)| ap.is_subseteq(bp));
        let b_sub_a = b
            .params
            .iter()
            .zip(a.params.iter())
            .all(|(bp, ap)| bp.is_subseteq(ap));

        match (a_sub_b, b_sub_a) {
            (true, true) => SignatureCmp::Equal,
            (true, false) => SignatureCmp::More, // a is more specific
            (false, true) => SignatureCmp::Less, // a is less specific
            (false, false) => SignatureCmp::Incomparable,
        }
    }
}
