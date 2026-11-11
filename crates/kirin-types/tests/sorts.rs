mod common;

use common::{Fixture, boolean, int, ty};
use kirin_test_utils::types::assert_eval_matches_signature;
use kirin_types::{
    BoolStaticCon, IntSort, IntSortT, IntStaticCon, SortError, SortValue, StaticSig,
};

#[test]
fn connectives_and_equality_have_sort_bool() {
    use BoolStaticCon::*;
    let mut f = Fixture::new();
    let (p, q, n) = (f.free(boolean()), f.free(boolean()), f.free(int()));
    let zero = f.lit(0);
    let not = f.bool_con(Not, vec![p]);
    let and = f.bool_con(And, vec![not, q]);
    let eq = f.bool_con(Equal(int()), vec![n, zero]);
    let implies = f.bool_con(Implies, vec![and, eq]);
    let or = f.bool_con(Or, vec![implies, p]);
    assert_eq!(f.check(or), Ok(boolean()));
    assert_eq!(
        f.statics.display(or, 0).to_string(),
        "∨(⇒(∧(¬(x0), x1), =(x2, Index(IntSortT(0)))), x0)"
    );
}

#[test]
fn equality_compares_terms_of_its_sort() {
    let mut f = Fixture::new();
    let (p, zero) = (f.free(boolean()), f.lit(0));
    let eq = f.bool_con(BoolStaticCon::Equal(int()), vec![p, zero]);
    assert_eq!(
        f.check(eq),
        Err(SortError::SortMismatch {
            term: p,
            shown: "x0".into(),
            expected: int(),
            found: boolean()
        })
    );
}

#[test]
fn bool_of_is_the_singleton_type_of_a_proposition() {
    let mut f = Fixture::new();
    let (n, zero) = (f.free(int()), f.lit(0));
    let geq = f.int_con(IntStaticCon::Geq, vec![n, zero]);
    let bool_of = f.bool_con(BoolStaticCon::BoolOf, vec![geq]);
    assert_eq!(f.check(bool_of), Ok(ty()));
    assert_eq!(
        f.statics.display(bool_of, 0).to_string(),
        "bool(≥(x0, Index(IntSortT(0))))"
    );
}

#[test]
fn equality_is_distinguished_by_its_sort() {
    let mut f = Fixture::new();
    let n = f.free(int());
    let eq_int = f.bool_con(BoolStaticCon::Equal(int()), vec![n, n]);
    let eq_bool = f.bool_con(BoolStaticCon::Equal(boolean()), vec![n, n]);
    assert!(!f.statics.term_eq(eq_int, eq_bool));
}

#[test]
fn int_of_is_the_singleton_type_of_an_integer() {
    let mut f = Fixture::new();
    let n = f.free(int());
    let int_of = f.int_con(IntStaticCon::IntOf, vec![n]);
    assert_eq!(f.check(int_of), Ok(ty()));
    assert_eq!(f.statics.display(int_of, 0).to_string(), "int(x0)");

    // its argument is an integer, not a type
    let token = f.token();
    let wrong = f.int_con(IntStaticCon::IntOf, vec![token]);
    assert_eq!(
        f.check(wrong),
        Err(SortError::SortMismatch {
            term: token,
            shown: "token".into(),
            expected: int(),
            found: ty()
        })
    );
}

#[test]
fn constructors_evaluate_across_sorts() {
    let (two, three) = (SortValue::Index(IntSortT(2)), SortValue::Index(IntSortT(3)));
    let int_eval =
        |con: IntStaticCon, args: &[SortValue<IntSortT>]| StaticSig::<IntSort>::eval(&con, args);
    let bool_eval = |con: BoolStaticCon<IntSort>, args: &[SortValue<IntSortT>]| con.eval(args);

    // int ⇒ int, and int ⇒ bool
    assert_eq!(
        int_eval(IntStaticCon::Add, &[two, three]),
        Some(SortValue::Index(IntSortT(5)))
    );
    assert_eq!(
        int_eval(IntStaticCon::Geq, &[two, three]),
        Some(SortValue::Bool(false))
    );
    // equality at `int` compares integers
    assert_eq!(
        bool_eval(BoolStaticCon::Equal(int()), &[three, three]),
        Some(SortValue::Bool(true))
    );
    // bool ⇒ bool
    assert_eq!(
        bool_eval(
            BoolStaticCon::Implies,
            &[SortValue::Bool(true), SortValue::Bool(false)]
        ),
        Some(SortValue::Bool(false))
    );
    // overflow, and types, don't fold
    let max = SortValue::Index(IntSortT(i64::MAX));
    assert_eq!(int_eval(IntStaticCon::Add, &[max, two]), None);
    assert_eq!(int_eval(IntStaticCon::IntOf, &[two]), None);
}

#[test]
fn eval_matches_signatures() {
    let (two, three) = (SortValue::Index(IntSortT(2)), SortValue::Index(IntSortT(3)));
    let (yes, no) = (SortValue::Bool(true), SortValue::Bool(false));
    for con in [IntStaticCon::Add, IntStaticCon::Sub, IntStaticCon::Geq] {
        assert_eval_matches_signature::<IntSort, _>(&con, &[two, three]);
    }
    assert_eval_matches_signature::<IntSort, _>(&IntStaticCon::Mul(4), &[three]);
    assert_eval_matches_signature::<IntSort, _>(&IntStaticCon::IntOf, &[three]);
    for con in [
        BoolStaticCon::And,
        BoolStaticCon::Or,
        BoolStaticCon::Implies,
    ] {
        assert_eval_matches_signature::<IntSort, _>(&con, &[yes, no]);
    }
    assert_eval_matches_signature::<IntSort, _>(&BoolStaticCon::Not, &[yes]);
    assert_eval_matches_signature::<IntSort, _>(&BoolStaticCon::BoolOf, &[no]);
    assert_eval_matches_signature::<IntSort, _>(&BoolStaticCon::Equal(int()), &[two, three]);
    assert_eval_matches_signature::<IntSort, _>(&BoolStaticCon::Equal(boolean()), &[yes, no]);
}
