mod common;

use common::{Fixture, boolean, int, ty};
use kirin_types::{IntStaticCon, SortError, StaticTerm};

#[test]
fn free_variables_have_their_recorded_sort() {
    let mut f = Fixture::new();
    let (n, b) = (f.free(int()), f.free(boolean()));
    assert_eq!(f.check(n), Ok(int()));
    assert_eq!(f.check(b), Ok(boolean()));
}

#[test]
fn dangling_bound_variable_is_unbound() {
    let mut f = Fixture::new();
    let var = f.bound(0);
    assert_eq!(
        f.check(var),
        Err(SortError::UnboundVar {
            term: var,
            index: 0,
            depth: 0
        })
    );
}

#[test]
fn nested_quantifiers_number_binders_innermost_first() {
    let mut f = Fixture::new();
    // ∀(n: int). ∃(b: bool). b ⊃ ((n ≥ 0) ∧ token) — b is Var(0), n is Var(1)
    let (b, n, zero, token) = (f.bound(0), f.bound(1), f.lit(0), f.token());
    let nonneg = f.int_con(IntStaticCon::Geq, vec![n, zero]);
    let asserted = f.term(StaticTerm::Assert(nonneg, token));
    let guarded = f.term(StaticTerm::Guard(b, asserted));
    let exists = f.term(StaticTerm::Exists(boolean(), guarded));
    let forall = f.term(StaticTerm::Forall(int(), exists));
    assert_eq!(f.check(forall), Ok(ty()));
}

#[test]
fn swapped_binders_are_rejected() {
    let mut f = Fixture::new();
    // ∀(n: int). ∃(b: bool). n ⊃ token — Var(1) is the int, not the bool
    let (n, token) = (f.bound(1), f.token());
    let guarded = f.term(StaticTerm::Guard(n, token));
    let exists = f.term(StaticTerm::Exists(boolean(), guarded));
    let forall = f.term(StaticTerm::Forall(int(), exists));
    assert_eq!(
        f.check(forall),
        Err(SortError::SortMismatch {
            term: n,
            shown: "a0".into(),
            expected: boolean(),
            found: int()
        })
    );
}

#[test]
fn quantifier_body_sees_free_variables() {
    let mut f = Fixture::new();
    // ∃(n: int). p ⊃ token, with p a free bool
    let (p, token) = (f.free(boolean()), f.token());
    let guarded = f.term(StaticTerm::Guard(p, token));
    let exists = f.term(StaticTerm::Exists(int(), guarded));
    assert_eq!(f.check(exists), Ok(ty()));
}

#[test]
fn var_beyond_a_quantifier_is_unbound() {
    let mut f = Fixture::new();
    let escaped = f.bound(1);
    let forall = f.term(StaticTerm::Forall(int(), escaped));
    assert_eq!(
        f.check(forall),
        Err(SortError::UnboundVar {
            term: escaped,
            index: 1,
            depth: 1
        })
    );
}

#[test]
fn constructor_result_comes_from_its_signature() {
    let mut f = Fixture::new();
    let one = f.lit(1);
    assert_eq!(f.check(one), Ok(int()));
    let two = f.lit(2);
    let sum = f.int_con(IntStaticCon::Add, vec![one, two]);
    assert_eq!(f.check(sum), Ok(int()));
    let geq = f.int_con(IntStaticCon::Geq, vec![sum, one]);
    assert_eq!(f.check(geq), Ok(boolean()));
}

#[test]
fn constructor_with_wrong_arity_is_rejected() {
    let mut f = Fixture::new();
    let one = f.lit(1);
    let add = f.int_con(IntStaticCon::Add, vec![one]);
    assert_eq!(
        f.check(add),
        Err(SortError::ArityMismatch {
            term: add,
            shown: "+(Index(IntSortT(1)))".into(),
            expected: 2,
            found: 1
        })
    );
}

#[test]
fn constructor_with_wrong_argument_sort_is_rejected() {
    let mut f = Fixture::new();
    let yes = f.bool_lit(true);
    let one = f.lit(1);
    let geq = f.int_con(IntStaticCon::Geq, vec![yes, one]);
    assert_eq!(
        f.check(geq),
        Err(SortError::SortMismatch {
            term: yes,
            shown: "Bool(true)".into(),
            expected: int(),
            found: boolean()
        })
    );
}

#[test]
fn bool_literals_have_sort_bool() {
    let mut f = Fixture::new();
    let no = f.bool_lit(false);
    assert_eq!(f.check(no), Ok(boolean()));
}

#[test]
fn forall_binds_var_zero_in_its_body() {
    let mut f = Fixture::new();
    // ∀(n: int). (n ≥ 0) ⊃ token
    let (n, zero) = (f.bound(0), f.lit(0));
    let guard = f.int_con(IntStaticCon::Geq, vec![n, zero]);
    let token = f.token();
    let guarded = f.term(StaticTerm::Guard(guard, token));
    let forall = f.term(StaticTerm::Forall(int(), guarded));
    assert_eq!(f.check(forall), Ok(ty()));
}

#[test]
fn quantifier_body_must_be_a_type() {
    let mut f = Fixture::new();
    // ∃(n: int). n ≥ 0 — the body is a bool, not a type
    let (n, zero) = (f.bound(0), f.lit(0));
    let geq = f.int_con(IntStaticCon::Geq, vec![n, zero]);
    let exists = f.term(StaticTerm::Exists(int(), geq));
    assert_eq!(
        f.check(exists),
        Err(SortError::SortMismatch {
            term: geq,
            shown: "≥(a0, Index(IntSortT(0)))".into(),
            expected: ty(),
            found: boolean()
        })
    );
}

#[test]
fn guard_and_assert_need_a_bool_then_a_type() {
    let mut f = Fixture::new();
    let (yes, token) = (f.bool_lit(true), f.token());
    let assert = f.term(StaticTerm::Assert(yes, token));
    assert_eq!(f.check(assert), Ok(ty()));

    let swapped = f.term(StaticTerm::Guard(token, yes));
    assert_eq!(
        f.check(swapped),
        Err(SortError::SortMismatch {
            term: token,
            shown: "token".into(),
            expected: boolean(),
            found: ty()
        })
    );
}

#[test]
fn product_and_function_types_take_types() {
    let mut f = Fixture::new();
    let token = f.token();
    let triple = f.term(StaticTerm::Product(vec![token, token, token]));
    assert_eq!(f.check(triple), Ok(ty()));
    let fun_ty = f.term(StaticTerm::Function(vec![token, triple], token));
    assert_eq!(f.check(fun_ty), Ok(ty()));
}

#[test]
fn function_type_result_must_be_a_type() {
    let mut f = Fixture::new();
    let (token, one) = (f.token(), f.lit(1));
    // (token, token) → 1: the result is argument 2, after the two parameters
    let fun_ty = f.term(StaticTerm::Function(vec![token, token], one));
    assert_eq!(
        f.check(fun_ty),
        Err(SortError::SortMismatch {
            term: one,
            shown: "Index(IntSortT(1))".into(),
            expected: ty(),
            found: int()
        })
    );
}

#[test]
fn terms_render_with_variables_named_by_level() {
    let mut f = Fixture::new();
    // ∀a0: int. ∃a1: bool. a1 ⊃ ((≥(a0, 0) ∧ token) * (token) → token)
    let (b, n, zero, token) = (f.bound(0), f.bound(1), f.lit(0), f.token());
    let nonneg = f.int_con(IntStaticCon::Geq, vec![n, zero]);
    let asserted = f.term(StaticTerm::Assert(nonneg, token));
    let thunk = f.term(StaticTerm::Function(vec![token], token));
    let pair = f.term(StaticTerm::Product(vec![asserted, thunk]));
    let guarded = f.term(StaticTerm::Guard(b, pair));
    let exists = f.term(StaticTerm::Exists(boolean(), guarded));
    let forall = f.term(StaticTerm::Forall(int(), exists));
    assert_eq!(
        f.statics.display(forall, 0).to_string(),
        "∀a0: int. (∃a1: bool. (a1 ⊃ ((≥(a0, Index(IntSortT(0))) ∧ token) * ((token) → token))))"
    );
    // the same body under a context of one variable, and an unbound variable
    assert_eq!(
        f.statics.display(nonneg, 2).to_string(),
        "≥(a0, Index(IntSortT(0)))"
    );
    let dangling = f.bound(3);
    assert_eq!(f.statics.display(dangling, 1).to_string(), "?3");
    let free = f.free(int());
    assert_eq!(f.statics.display(free, 0).to_string(), "x0");
}

#[test]
fn free_variables_render_by_their_name_hint() {
    let mut f = Fixture::new();
    let named = f.statics.fresh_var(int(), Some("n".into()));
    let n = f.term(StaticTerm::Free(named));
    let unnamed = f.free(int());
    let geq = f.int_con(IntStaticCon::Geq, vec![n, unnamed]);
    // a variable without a hint falls back to its id
    assert_eq!(f.statics.display(geq, 0).to_string(), "≥(n, x1)");
}

#[test]
fn errors_display_the_offending_term() {
    let mut f = Fixture::new();
    let (token, yes) = (f.token(), f.bool_lit(true));
    let swapped = f.term(StaticTerm::Guard(token, yes));
    assert_eq!(
        f.check(swapped).unwrap_err().to_string(),
        "`token` has sort `type`, but `bool` was expected"
    );
    let one = f.lit(1);
    let add = f.int_con(IntStaticCon::Add, vec![one]);
    assert_eq!(
        f.check(add).unwrap_err().to_string(),
        "`+(Index(IntSortT(1)))` expects 2 arguments, but got 1"
    );
}

#[test]
fn products_need_at_least_two_components() {
    let mut f = Fixture::new();
    let token = f.token();
    assert_eq!(f.check(token), Ok(ty()));
    let pair = f.term(StaticTerm::Product(vec![token, token]));
    assert_eq!(f.check(pair), Ok(ty()));

    let empty = f.term(StaticTerm::Product(vec![]));
    assert_eq!(
        f.check(empty),
        Err(SortError::ShortProduct {
            term: empty,
            shown: "()".into(),
            found: 0
        })
    );
    let single = f.term(StaticTerm::Product(vec![token]));
    assert_eq!(
        f.check(single).unwrap_err().to_string(),
        "a product needs at least 2 components, but `(token)` has 1"
    );
}

#[test]
fn term_eq_compares_structure_not_ids() {
    let mut f = Fixture::new();
    // two separately allocated copies of ∀(n: int). (n ≥ 0) ⊃ (token * token)
    let build = |f: &mut Fixture| {
        let (n, zero, token) = (f.bound(0), f.lit(0), f.token());
        let guard = f.int_con(IntStaticCon::Geq, vec![n, zero]);
        let pair = f.term(StaticTerm::Product(vec![token, token]));
        let guarded = f.term(StaticTerm::Guard(guard, pair));
        f.term(StaticTerm::Forall(int(), guarded))
    };
    let (a, b) = (build(&mut f), build(&mut f));
    assert_ne!(a, b);
    assert!(f.statics.term_eq(a, b));
}

#[test]
fn term_eq_distinguishes_different_terms() {
    let mut f = Fixture::new();
    let (token, zero, one) = (f.token(), f.lit(0), f.lit(1));
    let (x, y) = (f.bound(0), f.bound(1));
    let pair = f.term(StaticTerm::Product(vec![token, token]));
    let triple = f.term(StaticTerm::Product(vec![token, token, token]));
    let forall_int = f.term(StaticTerm::Forall(int(), token));
    let forall_bool = f.term(StaticTerm::Forall(boolean(), token));
    let exists_int = f.term(StaticTerm::Exists(int(), token));
    assert!(!f.statics.term_eq(zero, one)); // different constructors
    assert!(!f.statics.term_eq(x, y)); // different bound variables
    let (n, m) = (f.free(int()), f.free(int()));
    assert!(!f.statics.term_eq(n, m)); // different free variables
    assert!(!f.statics.term_eq(pair, triple)); // different arity
    assert!(!f.statics.term_eq(forall_int, forall_bool)); // different binder sort
    assert!(!f.statics.term_eq(forall_int, exists_int)); // different quantifier
    assert!(!f.statics.term_eq(token, pair)); // different shape
}

#[test]
fn mentions_finds_free_variables_under_binders() {
    let mut f = Fixture::new();
    let var = f.statics.fresh_var(boolean(), None);
    let (p, other, token) = (f.term(StaticTerm::Free(var)), f.free(boolean()), f.token());
    let guarded = f.term(StaticTerm::Guard(p, token));
    let forall = f.term(StaticTerm::Forall(int(), guarded));
    assert!(f.statics.mentions(forall, var));
    let unrelated = f.term(StaticTerm::Guard(other, token));
    assert!(!f.statics.mentions(unrelated, var));
}
