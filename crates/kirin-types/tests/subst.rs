mod common;

use common::{Fixture, TestCon, int, ty, types};
use kirin_types::{BaseSort, BoolStaticCon, IntSort, IntStaticCon, StaticTerm};

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
    let eq = f.bool_con(BoolStaticCon::Equal(BaseSort::Index(IntSort)), vec![m, a]);
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
    let pair = f.types(vec![token, token]);
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

#[test]
fn instantiate_replaces_a_list_variable_whole() {
    let mut f = Fixture::new();
    // [int(s), Tuple(τ̄)] under the quantifiers (s: int, τ̄: [type])
    let (s, tau, five, token) = (f.bound(1), f.bound(0), f.lit(5), f.token());
    let int_s = f.int_con(IntStaticCon::IntOf, vec![s]);
    let tuple = f.term(StaticTerm::Constructor(TestCon::Tuple, vec![tau]));
    let args = f.types(vec![int_s, tuple]);

    let unit = f.term(StaticTerm::Unit);
    let two = f.types(vec![token, unit]);
    let instantiated = f.statics.instantiate(args, &[five, two]);
    assert_eq!(
        f.statics.display(instantiated, 0).to_string(),
        "[int(Index(IntSortT(5))), Tuple([token, unit])]"
    );
    let int_five = f.int_con(IntStaticCon::IntOf, vec![five]);
    let expected_tuple = f.term(StaticTerm::Constructor(TestCon::Tuple, vec![two]));
    let expected = f.types(vec![int_five, expected_tuple]);
    assert!(f.statics.term_eq(instantiated, expected));
    assert_eq!(f.check(instantiated), Ok(types()));

    // the empty list
    let none = f.types(vec![]);
    let empty = f.statics.instantiate(args, &[five, none]);
    assert_eq!(
        f.statics.display(empty, 0).to_string(),
        "[int(Index(IntSortT(5))), Tuple([])]"
    );
}

#[test]
fn instantiate_replaces_a_bare_list_variable_with_the_list() {
    let mut f = Fixture::new();
    let (tau, token) = (f.bound(0), f.token());
    let list = f.types(vec![token, token]);
    assert_eq!(f.statics.instantiate(tau, &[list]), list);
}
