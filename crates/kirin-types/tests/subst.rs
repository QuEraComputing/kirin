mod common;

use common::{Fixture, int, ty};
use kirin_types::{BoolStaticCon, IntStaticCon, StaticTerm};

#[test]
fn instantiate_takes_arguments_outermost_first() {
    let mut f = Fixture::new();
    // ≥(Bound(1), Bound(0)) under the quantifiers (a, b), instantiated with (5, 3)
    let (a, b, five, three) = (f.bound(1), f.bound(0), f.lit(5), f.lit(3));
    let geq = f.int_con(IntStaticCon::Geq, vec![a, b]);
    let instantiated = f.statics.instantiate(geq, &[five, three]);
    assert_eq!(
        f.statics.display(instantiated, 0).to_string(),
        "≥(Index(IntSortT(5)), Index(IntSortT(3)))"
    );
}

#[test]
fn instantiate_skips_inner_binders_and_lowers_outer_indices() {
    let mut f = Fixture::new();
    // ∀(m: int). (m = Bound(1)) ⊃ token, with Bound(1) the eliminated
    // binder, and Bound(2) beyond it, which drops to Bound(1)
    let (m, a, beyond, n) = (f.bound(0), f.bound(1), f.bound(2), f.free(int()));
    let eq = f.bool_con(BoolStaticCon::Equal(int()), vec![m, a]);
    let token = f.token();
    let guarded = f.term(StaticTerm::Guard(eq, token));
    let forall = f.term(StaticTerm::Forall(int(), guarded));
    let instantiated = f.statics.instantiate(forall, &[n]);
    assert_eq!(
        f.statics.display(instantiated, 0).to_string(),
        "∀a0: int. (=(a0, x0) ⊃ token)"
    );

    let lowered = f.statics.instantiate(beyond, &[n]);
    let expected = f.bound(1);
    assert!(f.statics.term_eq(lowered, expected));
}

#[test]
fn instantiate_shares_terms_without_variables() {
    let mut f = Fixture::new();
    let (token, one) = (f.token(), f.lit(1));
    let pair = f.term(StaticTerm::Product(vec![token, token]));
    assert_eq!(f.statics.instantiate(pair, &[one]), pair);
}

#[test]
fn close_then_instantiate_gives_back_the_term() {
    let mut f = Fixture::new();
    // ∀(m: int). ≥(m, n) with n free
    let var = f.statics.fresh_var(int(), None);
    let (m, n) = (f.bound(0), f.term(StaticTerm::Free(var)));
    let geq = f.int_con(IntStaticCon::Geq, vec![m, n]);
    let token = f.token();
    let guarded = f.term(StaticTerm::Guard(geq, token));
    let forall = f.term(StaticTerm::Forall(int(), guarded));

    let closed = f.statics.close(forall, var);
    assert!(!f.statics.mentions(closed, var));
    let rebound = f.term(StaticTerm::Exists(int(), closed));
    assert_eq!(
        f.statics.display(rebound, 0).to_string(),
        "∃a0: int. (∀a1: int. (≥(a1, a0) ⊃ token))"
    );
    assert_eq!(f.check(rebound), Ok(ty()));

    let reopened = f.statics.instantiate(closed, &[n]);
    assert!(f.statics.term_eq(reopened, forall));
}
