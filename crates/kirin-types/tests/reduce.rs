mod common;

use std::fmt;

use common::{Fixture, int};
use kirin_types::{
    IntSort, IntSortT, IntStaticCon, Sort, SortError, SortValue, StaticCon, StaticSig, StaticTerm,
    Statics,
};

#[test]
fn reduce_folds_constructors_applied_to_literals() {
    let mut f = Fixture::new();
    // ≥(+(2, 3), 3) folds to `true`, through `5`
    let (two, three) = (f.lit(2), f.lit(3));
    let sum = f.int_con(IntStaticCon::Add, vec![two, three]);
    let geq = f.int_con(IntStaticCon::Geq, vec![sum, three]);

    let folded_sum = f.statics.reduce(sum).unwrap();
    let five = f.lit(5);
    assert!(f.statics.term_eq(folded_sum, five));

    let folded = f.statics.reduce(geq).unwrap();
    let yes = f.bool_lit(true);
    assert!(f.statics.term_eq(folded, yes));
}

#[test]
fn reduce_stops_at_variables_and_folds_around_them() {
    let mut f = Fixture::new();
    // ∀n: int. ≥(n, +(1, 1)) folds to ∀n: int. ≥(n, 2)
    let (n, one) = (f.bound(0), f.lit(1));
    let sum = f.int_con(IntStaticCon::Add, vec![one, one]);
    let geq = f.int_con(IntStaticCon::Geq, vec![n, sum]);
    let token = f.token();
    let guarded = f.term(StaticTerm::Guard(geq, token));
    let forall = f.term(StaticTerm::Forall(int(), guarded));

    let folded = f.statics.reduce(forall).unwrap();
    let two = f.lit(2);
    let expected_geq = f.int_con(IntStaticCon::Geq, vec![n, two]);
    let expected_guard = f.term(StaticTerm::Guard(expected_geq, token));
    let expected = f.term(StaticTerm::Forall(int(), expected_guard));
    assert!(f.statics.term_eq(folded, expected));
}

#[test]
fn reduce_shares_terms_it_cannot_fold() {
    let mut f = Fixture::new();
    // overflow leaves `+(max, 1)` as it is, and a free variable blocks folding
    let (max, one, x) = (f.lit(i64::MAX), f.lit(1), f.free(int()));
    let overflow = f.int_con(IntStaticCon::Add, vec![max, one]);
    assert_eq!(f.statics.reduce(overflow), Ok(overflow));
    let symbolic = f.int_con(IntStaticCon::Add, vec![x, one]);
    assert_eq!(f.statics.reduce(symbolic), Ok(symbolic));
}

#[test]
fn reduce_checks_the_arity_before_evaluating() {
    let mut f = Fixture::new();
    let one = f.lit(1);
    let add = f.int_con(IntStaticCon::Add, vec![one]);
    assert!(matches!(
        f.statics.reduce(add),
        Err(SortError::ArityMismatch {
            expected: 2,
            found: 1,
            ..
        })
    ));
}

/// `liar : () ⇒ int`, whose `eval` wrongly gives a `bool`.
#[derive(Clone, PartialEq)]
struct Liar;

impl StaticSig<IntSort> for Liar {
    fn signature(&self) -> StaticCon<IntSort> {
        StaticCon {
            args: vec![],
            result: int(),
        }
    }

    fn eval(&self, _: &[SortValue<IntSortT>]) -> Option<SortValue<IntSortT>> {
        Some(SortValue::Bool(true))
    }
}

impl fmt::Display for Liar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("liar")
    }
}

#[test]
fn reduce_rejects_a_value_of_the_wrong_sort() {
    let mut statics = Statics::<IntSort, Liar>::default();
    let liar = statics.alloc(StaticTerm::Constructor(Liar, vec![]));
    assert_eq!(
        statics.reduce(liar),
        Err(SortError::EvalSort {
            term: liar,
            shown: "liar".into(),
            expected: int(),
            found: Sort::Bool
        })
    );
    assert_eq!(
        statics.reduce(liar).unwrap_err().to_string(),
        "`liar` evaluates to a value of sort `bool`, but its signature's result is `int`"
    );
}
