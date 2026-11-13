use crate::SortError;

use super::{IndexSort, StaticSig, StaticTerm, StaticTermId, Statics};

impl<S: IndexSort, C: StaticSig<S>> Statics<S, C> {
    /// Folds constants in `term`: traverses `term` bottom-up, every
    /// constructor application whose arguments are all literals is replaced
    /// by the literal it evaluates to, if its [`StaticSig::eval`] gives one.
    ///
    /// Fails if a constructor applied to literals has the wrong number of
    /// arguments, or evaluates to a value of another sort than its
    /// signature's result.
    pub fn reduce(&mut self, term: StaticTermId) -> Result<StaticTermId, SortError<S>> {
        self.map_term(term, 0, &|statics, original, node, _| {
            statics.reduce_constructor(original, node)
        })
    }

    /// Reduces any [`StaticTerm::Constructor`] to the value it evaluates to
    /// if all its args are [`StaticTerm::Lit`].
    fn reduce_constructor(
        &mut self,
        term: StaticTermId,
        node: &StaticTerm<S, C>,
    ) -> Result<Option<StaticTermId>, SortError<S>> {
        let StaticTerm::Constructor(con, args) = node else {
            return Ok(None);
        };

        // Get the values of the literal args
        let Some(values) = args
            .iter()
            .map(|arg| match &*self.terms[*arg] {
                StaticTerm::Lit(value) => Some(value.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
        else {
            // return early if one of the args was not a `Lit`
            return Ok(None);
        };

        // Number of arguments must match the signature's
        let sig = con.signature();
        if values.len() != sig.args.len() {
            return Err(SortError::ArityMismatch {
                term,
                shown: self.display(term, 0).to_string(),
                expected: sig.args.len(),
                found: values.len(),
            });
        }

        let Some(value) = con.eval(&values) else {
            // Some constructors do not evaluate
            return Ok(None);
        };

        // The result value's sort must match the signature's
        let found = value.sort();
        if found != sig.result {
            return Err(SortError::EvalSort {
                term,
                shown: self.display(term, 0).to_string(),
                expected: sig.result,
                found,
            });
        }

        Ok(Some(self.alloc(StaticTerm::Lit(value))))
    }
}
