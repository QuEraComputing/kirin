use std::marker::PhantomData;

use crate::lattice::TypeLattice;

use super::definition::Signature;

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
        if call.params().len() != cand.params().len() {
            return None;
        }
        let params_match = call
            .params()
            .iter()
            .zip(cand.params().iter())
            .all(|(a, b)| a == b);
        #[allow(clippy::unit_cmp)]
        if params_match && call.ret() == cand.ret() && call.constraints() == cand.constraints() {
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
        if call.params().len() != cand.params().len() {
            return None;
        }
        #[allow(clippy::unit_cmp)]
        if call.constraints() != cand.constraints() {
            return None;
        }
        // Call return type must match candidate return type if specified
        if call.ret() != cand.ret() {
            return None;
        }
        // Call params must be subtypes (more specific or equal) of candidate params
        let all_applicable = call
            .params()
            .iter()
            .zip(cand.params().iter())
            .all(|(call_param, cand_param)| call_param.is_subseteq(cand_param));
        if all_applicable { Some(()) } else { None }
    }

    fn cmp_candidate(
        a: &Signature<T>,
        _a_env: &Self::Env,
        b: &Signature<T>,
        _b_env: &Self::Env,
    ) -> SignatureCmp {
        if a.params().len() != b.params().len() {
            return SignatureCmp::Incomparable;
        }

        let a_sub_b = a
            .params()
            .iter()
            .zip(b.params().iter())
            .all(|(ap, bp)| ap.is_subseteq(bp));
        let b_sub_a = b
            .params()
            .iter()
            .zip(a.params().iter())
            .all(|(bp, ap)| bp.is_subseteq(ap));

        match (a_sub_b, b_sub_a) {
            (true, true) => SignatureCmp::Equal,
            (true, false) => SignatureCmp::More, // a is more specific
            (false, true) => SignatureCmp::Less, // a is less specific
            (false, false) => SignatureCmp::Incomparable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lattice::{HasBottom, HasTop, Lattice};

    /// A simple 3-element lattice: Bot < Mid < Top
    #[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
    enum SimpleType {
        Bot,
        #[default]
        Mid,
        Top,
    }

    impl Lattice for SimpleType {
        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (SimpleType::Top, _) | (_, SimpleType::Top) => SimpleType::Top,
                (SimpleType::Mid, _) | (_, SimpleType::Mid) => SimpleType::Mid,
                _ => SimpleType::Bot,
            }
        }

        fn meet(&self, other: &Self) -> Self {
            match (self, other) {
                (SimpleType::Bot, _) | (_, SimpleType::Bot) => SimpleType::Bot,
                (SimpleType::Mid, _) | (_, SimpleType::Mid) => SimpleType::Mid,
                _ => SimpleType::Top,
            }
        }

        fn is_subseteq(&self, other: &Self) -> bool {
            matches!(
                (self, other),
                (SimpleType::Bot, _)
                    | (SimpleType::Mid, SimpleType::Mid)
                    | (SimpleType::Mid, SimpleType::Top)
                    | (SimpleType::Top, SimpleType::Top)
            )
        }
    }

    impl HasBottom for SimpleType {
        fn bottom() -> Self {
            SimpleType::Bot
        }
    }

    impl HasTop for SimpleType {
        fn top() -> Self {
            SimpleType::Top
        }
    }

    impl TypeLattice for SimpleType {}

    fn make_sig(params: Vec<SimpleType>, ret: SimpleType) -> Signature<SimpleType> {
        Signature::new(params, ret, ())
    }

    #[test]
    fn lattice_semantics_applicable_subtype() {
        let call = make_sig(vec![SimpleType::Bot], SimpleType::Mid);
        let cand = make_sig(vec![SimpleType::Mid], SimpleType::Mid);

        // Bot is_subseteq Mid, so call is applicable to cand
        assert!(LatticeSemantics::<SimpleType>::applicable(&call, &cand).is_some());
    }

    #[test]
    fn lattice_semantics_not_applicable_supertype() {
        let call = make_sig(vec![SimpleType::Top], SimpleType::Mid);
        let cand = make_sig(vec![SimpleType::Mid], SimpleType::Mid);

        // Top is NOT is_subseteq Mid
        assert!(LatticeSemantics::<SimpleType>::applicable(&call, &cand).is_none());
    }

    #[test]
    fn lattice_semantics_not_applicable_arity_mismatch() {
        let call = make_sig(vec![SimpleType::Bot, SimpleType::Bot], SimpleType::Mid);
        let cand = make_sig(vec![SimpleType::Mid], SimpleType::Mid);

        assert!(LatticeSemantics::<SimpleType>::applicable(&call, &cand).is_none());
    }

    #[test]
    fn lattice_semantics_cmp_more_specific() {
        let a = make_sig(vec![SimpleType::Bot], SimpleType::Mid);
        let b = make_sig(vec![SimpleType::Mid], SimpleType::Mid);

        // a (Bot) is more specific than b (Mid)
        assert_eq!(
            LatticeSemantics::<SimpleType>::cmp_candidate(&a, &(), &b, &()),
            SignatureCmp::More
        );
    }

    #[test]
    fn lattice_semantics_cmp_less_specific() {
        let a = make_sig(vec![SimpleType::Top], SimpleType::Mid);
        let b = make_sig(vec![SimpleType::Mid], SimpleType::Mid);

        assert_eq!(
            LatticeSemantics::<SimpleType>::cmp_candidate(&a, &(), &b, &()),
            SignatureCmp::Less
        );
    }

    #[test]
    fn lattice_semantics_cmp_equal() {
        let a = make_sig(vec![SimpleType::Mid], SimpleType::Mid);
        let b = make_sig(vec![SimpleType::Mid], SimpleType::Mid);

        assert_eq!(
            LatticeSemantics::<SimpleType>::cmp_candidate(&a, &(), &b, &()),
            SignatureCmp::Equal
        );
    }

    #[test]
    fn exact_semantics_applicable_and_cmp() {
        let sig1 = Signature::new(vec![1, 2], 3, ());
        let sig2 = Signature::new(vec![1, 2], 3, ());
        assert!(ExactSemantics::applicable(&sig1, &sig2).is_some());
        assert_eq!(
            ExactSemantics::cmp_candidate(&sig1, &(), &sig2, &()),
            SignatureCmp::Equal
        );

        let sig3 = Signature::new(vec![1, 99], 3, ());
        assert!(ExactSemantics::applicable(&sig1, &sig3).is_none());
    }
}
