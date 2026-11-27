mod common;

use common::{Fixture, TestCon, boolean, int, ty, types};
use kirin_types::{
    BaseSort, BoolStaticCon, IntSort, IntStaticCon, SortError, StaticTerm, StaticTermId, TypeSig,
};

/// The number of items in the list `term`.
fn len(f: &Fixture, term: StaticTermId) -> usize {
    match &*f.statics.terms[term] {
        StaticTerm::Seq(_, items) => items.len(),
        other => panic!("{other:?} is not a list"),
    }
}

/// A signature with no existentials or asserts.
fn universal(
    quantifiers: Vec<common::TestSort>,
    guards: Vec<StaticTermId>,
    args: Vec<StaticTermId>,
    results: Vec<StaticTermId>,
) -> TypeSig<IntSort> {
    TypeSig {
        quantifiers,
        guards,
        args,
        existentials: vec![],
        asserts: vec![],
        results,
    }
}

/// `scf.for : ∀s e z: int. ∀τ̄: [type]. [z ≥ 1] ⊃ [int(s), int(e), int(z), τ̄] ⇒ [τ̄]`
fn scf_for(f: &mut Fixture) -> TypeSig<IntSort> {
    let (s, e, z, tau, one) = (f.bound(3), f.bound(2), f.bound(1), f.bound(0), f.lit(1));
    let positive = f.int_con(IntStaticCon::Geq, vec![z, one]);
    let [int_s, int_e, int_z] = [s, e, z].map(|n| f.int_con(IntStaticCon::IntOf, vec![n]));
    universal(
        vec![int(), int(), int(), types()],
        vec![positive],
        vec![int_s, int_e, int_z, tau],
        vec![tau],
    )
}

#[test]
fn scf_for_carries_any_number_of_values() {
    let mut f = Fixture::new();
    let sig = scf_for(&mut f);
    assert_eq!(f.statics.check_sig(&sig), Ok(()));

    let (start, end, step, token) = (f.lit(0), f.lit(10), f.lit(1), f.token());
    for carried in 0..3 {
        let list = f.types(vec![token; carried]);
        let static_args = [start, end, step, list];
        let args: Vec<_> = sig
            .args
            .iter()
            .map(|arg| f.statics.instantiate(*arg, &static_args))
            .collect();
        // one entry per operand field: `init_args` is the last one
        assert_eq!(args.len(), 4);
        assert_eq!(f.check(args[0]), Ok(ty()));
        assert_eq!(len(&f, args[3]), carried);
        let results = f.statics.instantiate(sig.results[0], &static_args);
        assert_eq!(len(&f, results), carried);
    }
}

#[test]
fn scf_if_returns_any_number_of_values() {
    let mut f = Fixture::new();
    // ∀τ̄: [type]. ∀b: bool. [bool(b)] ⇒ [τ̄]
    let (tau, b) = (f.bound(1), f.bound(0));
    let bool_b = f.bool_con(BoolStaticCon::BoolOf, vec![b]);
    let sig = universal(vec![types(), boolean()], vec![], vec![bool_b], vec![tau]);
    assert_eq!(f.statics.check_sig(&sig), Ok(()));
}

#[test]
fn existentials_relate_the_results() {
    let mut f = Fixture::new();
    // split : ∀a: type. ∀n: int. [Tensor(a, [n])]
    //           ⇒ ∃k: int. ∃j: int. [k + j = n] ∧ [Tensor(a, [k]), Tensor(a, [j])]
    let int_sort = BaseSort::Index(IntSort);
    let tensor = |f: &mut Fixture, a, n| {
        let shape = f.term(StaticTerm::Seq(int_sort, vec![n]));
        f.term(StaticTerm::Constructor(TestCon::Tensor, vec![a, shape]))
    };
    let (a, n) = (f.bound(1), f.bound(0));
    let whole = tensor(&mut f, a, n);

    // under the existentials too: a and n move out to Bound(3) and Bound(2)
    let (a, n, k, j) = (f.bound(3), f.bound(2), f.bound(1), f.bound(0));
    let sum = f.int_con(IntStaticCon::Add, vec![k, j]);
    let split = f.bool_con(BoolStaticCon::Equal(int_sort), vec![sum, n]);
    let (left, right) = (tensor(&mut f, a, k), tensor(&mut f, a, j));

    let sig = TypeSig {
        quantifiers: vec![ty(), int()],
        guards: vec![],
        args: vec![whole],
        existentials: vec![int(), int()],
        asserts: vec![split],
        results: vec![left, right],
    };
    assert_eq!(f.statics.check_sig(&sig), Ok(()));

    // a use: the static arguments, then fresh rigid witnesses
    let (token, ten) = (f.token(), f.lit(10));
    let k = f.statics.fresh_var(int(), Some("k".into()));
    let j = f.statics.fresh_var(int(), Some("j".into()));
    let (k, j) = (f.term(StaticTerm::Free(k)), f.term(StaticTerm::Free(j)));
    let opened = [token, ten, k, j];
    let assumed = f.statics.instantiate(sig.asserts[0], &opened);
    assert_eq!(
        f.statics.display(assumed, 0).to_string(),
        "=(+(k, j), Index(IntSortT(10)))"
    );
    let right = f.statics.instantiate(sig.results[1], &opened);
    assert_eq!(
        f.statics.display(right, 0).to_string(),
        "Tensor(token, [j])"
    );
}

#[test]
fn guards_and_args_do_not_see_the_existentials() {
    let mut f = Fixture::new();
    let b = f.bound(0);
    let sig = TypeSig {
        quantifiers: vec![],
        guards: vec![b],
        args: vec![],
        existentials: vec![boolean()],
        asserts: vec![b],
        results: vec![],
    };
    assert_eq!(
        f.statics.check_sig(&sig),
        Err(SortError::UnboundVar {
            term: b,
            index: 0,
            depth: 0
        })
    );
    // the same variable is fine as an assert
    let sig = TypeSig {
        guards: vec![],
        ..sig
    };
    assert_eq!(f.statics.check_sig(&sig), Ok(()));
}

#[test]
fn entries_are_types_or_lists_of_types() {
    let mut f = Fixture::new();
    let (one, token) = (f.lit(1), f.token());
    let list = f.types(vec![token]);
    let sig = universal(vec![], vec![], vec![token, list], vec![one]);
    assert_eq!(
        f.statics.check_sig(&sig),
        Err(SortError::SortMismatch {
            term: one,
            shown: "Index(IntSortT(1))".into(),
            expected: ty(),
            found: int()
        })
    );
}

#[test]
fn guards_are_propositions_under_the_quantifiers() {
    let mut f = Fixture::new();
    let n = f.bound(0);
    let sig = universal(vec![int()], vec![n], vec![], vec![]);
    assert_eq!(
        f.statics.check_sig(&sig),
        Err(SortError::SortMismatch {
            term: n,
            shown: "a0".into(),
            expected: boolean(),
            found: int()
        })
    );
}

#[test]
fn entries_are_not_quantified_guarded_or_asserting() {
    let mut f = Fixture::new();
    let (token, yes) = (f.token(), f.bool_lit(true));
    let forall = f.term(StaticTerm::Forall(int(), token));
    let exists = f.term(StaticTerm::Exists(int(), token));
    let guarded = f.term(StaticTerm::Guard(yes, token));
    let asserted = f.term(StaticTerm::Assert(yes, token));
    for entry in [forall, exists, guarded, asserted] {
        let as_arg = universal(vec![], vec![], vec![entry], vec![]);
        let as_result = universal(vec![], vec![], vec![], vec![entry]);
        for sig in [as_arg, as_result] {
            assert!(matches!(
                f.statics.check_sig(&sig),
                Err(SortError::NotPrenex { term, .. }) if term == entry
            ));
        }
    }

    // nor is an item of a list entry
    let list = f.types(vec![token, guarded]);
    let sig = universal(vec![], vec![], vec![list], vec![]);
    assert_eq!(
        f.statics.check_sig(&sig),
        Err(SortError::NotPrenex {
            term: guarded,
            shown: "Bool(true) ⊃ token".into(),
        })
    );
}

#[test]
fn entries_contain_no_binders_at_any_depth() {
    let mut f = Fixture::new();
    // Tuple([∀i: int. Fn([int(i)], [])]): the binder is under a function type,
    // a list and a constructor
    let i = f.bound(0);
    let int_i = f.int_con(IntStaticCon::IntOf, vec![i]);
    let (args, nothing) = (f.types(vec![int_i]), f.types(vec![]));
    let fun_ty = f.fun(args, nothing);
    let body_ty = f.term(StaticTerm::Forall(int(), fun_ty));
    let tuple_items = f.types(vec![body_ty]);
    let tuple = f.term(StaticTerm::Constructor(TestCon::Tuple, vec![tuple_items]));
    let sig = universal(vec![], vec![], vec![tuple], vec![]);
    assert_eq!(
        f.statics.check_sig(&sig),
        Err(SortError::NotPrenex {
            term: body_ty,
            shown: "∀a0: int. Fn([int(a0)], [])".into(),
        })
    );

    // a guard in a function type's results
    let (token, yes) = (f.token(), f.bool_lit(true));
    let guarded = f.term(StaticTerm::Guard(yes, token));
    let results = f.types(vec![guarded]);
    let fun_ty = f.fun(nothing, results);
    let sig = universal(vec![], vec![], vec![], vec![fun_ty]);
    assert!(matches!(
        f.statics.check_sig(&sig),
        Err(SortError::NotPrenex { term, .. }) if term == guarded
    ));

    // the same function types without binders are fine
    let plain = f.fun(args, nothing);
    let sig = universal(vec![int()], vec![], vec![plain], vec![]);
    assert_eq!(f.statics.check_sig(&sig), Ok(()));
}
