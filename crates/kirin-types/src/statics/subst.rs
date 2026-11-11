use std::convert::Infallible;

use super::{IndexSort, StaticTerm, StaticTermId, StaticVarId, Statics};

impl<B: IndexSort, C: Clone> Statics<B, C> {
    /// Apply static arguments to a binder term's body. For example,
    ///   - `∀x:σ. T': Static argument `a` substitutes for `x` in `T`
    ///   - `∃x:σ. T': Static argument `a` substitutes for `x` in `T`
    ///   - [`DynSig`](crate::DynSig): Static arguments from
    ///     [`DynTerm::Constructor`](crate::DynTerm::Constructor) substitute
    ///     for the quantifiers in the signature's guards, arguments, and results.
    ///
    ///
    /// The caller is expected to remove the binder term itself(e.g.
    /// [`StaticTerm::Forall`] or [`StaticTerm::Exists`]).
    ///
    /// This function
    ///  1. Replaces `term`'s bound variables with the corresponding argument
    ///     supplied in `args`: `Bound(i)` is replaced by ith arg from the
    ///     back in `args`,
    ///  2. Shifts all other bound variables by the number of arguments
    ///     i.e. 1 in the case of Forall/Exists, and args.len() in the case of
    ///     a `DynSig`'s quantifiers.
    pub fn instantiate(&mut self, term: StaticTermId, args: &[StaticTermId]) -> StaticTermId {
        let k = args.len();

        // Applies the corresponding argument to `term`'s bound variables
        // and shifts all other bound variables.
        let instantiate_bound_with_arg =
            |statics: &mut Statics<B, C>, node: &StaticTerm<B, C>, depth| {
                let StaticTerm::Bound(i) = *node else {
                    return None;
                };
                match i.checked_sub(depth) {
                    None => None,
                    Some(level) if level < k => Some(args[k - 1 - level]),
                    Some(_) => Some(statics.alloc(StaticTerm::Bound(i - k))),
                }
            };

        let Ok(instantiated) = self.map_term::<Infallible>(term, 0, &|statics, _, node, depth| {
            Ok(instantiate_bound_with_arg(statics, node, depth))
        });
        instantiated
    }

    /// Replace `Free(var)` with `Bound(d)`, where `d` is its depth in `term`.
    /// The caller should then wrap the resulting `term` in a binder.
    pub fn close(&mut self, term: StaticTermId, var: StaticVarId) -> StaticTermId {
        let Ok(closed) = self.map_term::<Infallible>(term, 0, &|statics, _, node, depth| {
            Ok(match *node {
                StaticTerm::Free(x) if x == var => Some(statics.alloc(StaticTerm::Bound(depth))),
                _ => None,
            })
        });
        closed
    }
}
