//! Checks for the static constructors of `kirin-types`.

use kirin_types::{IndexSort, SortValue, StaticSig};

/// Asserts that `con`, applied to the sample `args`, evaluates consistently
/// with its signature: `args` must match the signature's argument sorts, and
/// a value `eval` returns must have the signature's result sort.
pub fn assert_eval_matches_signature<S, C>(con: &C, args: &[SortValue<S::T>])
where
    S: IndexSort,
    C: StaticSig<S>,
{
    let sig = con.signature();
    assert_eq!(
        args.len(),
        sig.args.len(),
        "`{con}` takes {} arguments, but the sample has {}",
        sig.args.len(),
        args.len()
    );
    for (arg, expected) in args.iter().zip(&sig.args) {
        assert_eq!(
            arg.sort::<S>(),
            *expected,
            "the sample argument {arg:?} of `{con}` has the wrong sort"
        );
    }
    if let Some(value) = con.eval(args) {
        assert_eq!(
            value.sort::<S>(),
            sig.result,
            "`{con}` evaluates {args:?} to {value:?}, which isn't of its result sort"
        );
    }
}
