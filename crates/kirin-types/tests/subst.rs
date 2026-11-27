mod common;

use common::{Fixture, int, ty};
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
    // Fn([int(s)], τ̄) under the quantifiers (s: int, τ̄: [type])
    let (s, tau, five, token) = (f.bound(1), f.bound(0), f.lit(5), f.token());
    let int_s = f.int_con(IntStaticCon::IntOf, vec![s]);
    let args = f.types(vec![int_s]);
    let fun_ty = f.fun(args, tau);

    let unit = f.unit();
    let two = f.types(vec![token, unit]);
    let instantiated = f.statics.instantiate(fun_ty, &[five, two]);
    assert_eq!(
        f.statics.display(instantiated, 0).to_string(),
        "Fn([int(Index(IntSortT(5)))], [token, unit])"
    );
    let int_five = f.int_con(IntStaticCon::IntOf, vec![five]);
    let expected_args = f.types(vec![int_five]);
    let expected = f.fun(expected_args, two);
    assert!(f.statics.term_eq(instantiated, expected));
    assert_eq!(f.check(instantiated), Ok(ty()));

    // the empty list
    let none = f.types(vec![]);
    let no_results = f.statics.instantiate(fun_ty, &[five, none]);
    assert_eq!(
        f.statics.display(no_results, 0).to_string(),
        "Fn([int(Index(IntSortT(5)))], [])"
    );
}

#[test]
fn instantiate_replaces_a_bare_list_variable_with_the_list() {
    let mut f = Fixture::new();
    let (tau, token) = (f.bound(0), f.token());
    let list = f.types(vec![token, token]);
    assert_eq!(f.statics.instantiate(tau, &[list]), list);
}

#[test]
fn instantiate_replaces_list_variables_under_inner_binders() {
    let mut f = Fixture::new();
    // ∀i: int. Fn([int(i)], τ̄), with τ̄ the eliminated binder
    let (i, tau, token) = (f.bound(0), f.bound(1), f.token());
    let int_i = f.int_con(IntStaticCon::IntOf, vec![i]);
    let args = f.types(vec![int_i]);
    let body = f.fun(args, tau);
    let forall = f.term(StaticTerm::Forall(int(), body));

    let carried = f.types(vec![token]);
    let instantiated = f.statics.instantiate(forall, &[carried]);
    assert_eq!(
        f.statics.display(instantiated, 0).to_string(),
        "∀a0: int. Fn([int(a0)], [token])"
    );
    assert_eq!(f.check(instantiated), Ok(ty()));
}
