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
/// all languages in the program should use the same `SignatureSemantics`
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

    /// Does a specialization's signature fit the interface its staged
    /// function declares?
    ///
    /// This is a different relation from [`applicable`](Self::applicable),
    /// which matches a call site against a candidate. A specialization may be
    /// narrower than its interface in every position, returns included.
    ///
    /// The default requires the specialization to be applicable where the
    /// interface is, which keeps return types equal.
    fn refines(spec: &Signature<T, C>, interface: &Signature<T, C>) -> bool {
        Self::applicable(spec, interface).is_some()
    }
}

/// Exact-match semantics: applicable only when params and rets are exactly equal.
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
        if params_match && call.rets() == cand.rets() && call.constraints() == cand.constraints() {
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

/// Shape-only semantics, for a dialect that declares no order on its types.
///
/// A specialization fits its interface when the parameter and return counts
/// agree; the types are not compared. Dispatch is exact, as in
/// [`ExactSemantics`]. This is what `#[derive(Dialect)]` selects unless the
/// dialect names another semantics with `#[kirin(signatures = ...)]`.
pub struct ShapeSemantics;

impl<T: PartialEq, C: PartialEq> SignatureSemantics<T, C> for ShapeSemantics {
    type Env = ();

    fn applicable(call: &Signature<T, C>, cand: &Signature<T, C>) -> Option<Self::Env> {
        ExactSemantics::applicable(call, cand)
    }

    fn cmp_candidate(
        a: &Signature<T, C>,
        a_env: &Self::Env,
        b: &Signature<T, C>,
        b_env: &Self::Env,
    ) -> SignatureCmp {
        ExactSemantics::cmp_candidate(a, a_env, b, b_env)
    }

    fn refines(spec: &Signature<T, C>, interface: &Signature<T, C>) -> bool {
        spec.params().len() == interface.params().len()
            && spec.rets().len() == interface.rets().len()
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
        // Call return types must match candidate return types
        if call.rets() != cand.rets() {
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

    /// Same shape, and every parameter and return type is a subset of or
    /// equal to the interface's.
    fn refines(spec: &Signature<T>, interface: &Signature<T>) -> bool {
        #[allow(clippy::unit_cmp)]
        let same_constraints = spec.constraints() == interface.constraints();
        let pointwise = |spec: &crate::Product<T>, interface: &crate::Product<T>| {
            spec.len() == interface.len()
                && spec.iter().zip(interface).all(|(s, i)| s.is_subseteq(i))
        };
        same_constraints
            && pointwise(spec.params(), interface.params())
            && pointwise(spec.rets(), interface.rets())
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

    impl std::fmt::Display for SimpleType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                SimpleType::Bot => write!(f, "Bot"),
                SimpleType::Mid => write!(f, "Mid"),
                SimpleType::Top => write!(f, "Top"),
            }
        }
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
        Signature::new().takes(params).returns(ret)
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
    fn lattice_semantics_refines_allows_narrower_params_and_returns() {
        let interface = make_sig(vec![SimpleType::Top], SimpleType::Top);
        let narrower = make_sig(vec![SimpleType::Mid], SimpleType::Mid);
        assert!(LatticeSemantics::<SimpleType>::refines(
            &narrower, &interface
        ));
        assert!(LatticeSemantics::<SimpleType>::refines(
            &interface, &interface
        ));
        // A narrower return is not a matching call: dispatch keeps returns equal.
        assert!(LatticeSemantics::<SimpleType>::applicable(&narrower, &interface).is_none());
    }

    #[test]
    fn lattice_semantics_refines_rejects_wider_types_and_other_shapes() {
        let interface = make_sig(vec![SimpleType::Mid], SimpleType::Mid);
        let wider_param = make_sig(vec![SimpleType::Top], SimpleType::Mid);
        let wider_ret = make_sig(vec![SimpleType::Mid], SimpleType::Top);
        let extra_param = make_sig(vec![SimpleType::Mid, SimpleType::Mid], SimpleType::Mid);
        let extra_ret = Signature::new()
            .takes([SimpleType::Mid])
            .returns_each([SimpleType::Mid, SimpleType::Mid]);
        for spec in [wider_param, wider_ret, extra_param, extra_ret] {
            assert!(!LatticeSemantics::<SimpleType>::refines(&spec, &interface));
        }
    }

    #[test]
    fn exact_semantics_refines_only_an_equal_signature() {
        let interface = make_sig(vec![SimpleType::Mid], SimpleType::Mid);
        let narrower = make_sig(vec![SimpleType::Bot], SimpleType::Mid);
        assert!(ExactSemantics::refines(&interface, &interface));
        assert!(!ExactSemantics::refines(&narrower, &interface));
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
        let sig1 = Signature::new().takes([1, 2]).returns(3);
        let sig2 = Signature::new().takes([1, 2]).returns(3);
        assert!(ExactSemantics::applicable(&sig1, &sig2).is_some());
        assert_eq!(
            ExactSemantics::cmp_candidate(&sig1, &(), &sig2, &()),
            SignatureCmp::Equal
        );

        let sig3 = Signature::new().takes([1, 99]).returns(3);
        assert!(ExactSemantics::applicable(&sig1, &sig3).is_none());
    }
}
