use crate::SortError;

use super::{BaseSort, IndexSort, Sort, StaticSig, StaticTerm, StaticTermId, Statics, TypeSig};

impl<B: IndexSort, C: StaticSig<B>> Statics<B, C> {
    /// The sort of the locally closed `term`.
    /// Locally closed means that there are no `Bound(i)` variables without a
    /// corresponding enclosing binder.
    pub fn check_sort(&self, term: StaticTermId) -> Result<Sort<B>, SortError<B>> {
        self.sort_under(term, &mut Vec::new())
    }

    /// Checks that `sig` is well-formed: every guard and assert has sort
    /// `bool`, and every argument and result has sort `type` or `[type]`.
    /// Guards and arguments are checked under the quantifiers; asserts and
    /// results under the quantifiers and then the existentials.
    ///
    /// The signature must be in prenex form: no argument or result may
    /// contain a `∀`, `∃`, `⊃` or `∧` anywhere; those belong in the
    /// signature's own fields.
    pub fn check_sig(&self, sig: &TypeSig<B>) -> Result<(), SortError<B>> {
        let context = &mut sig.quantifiers.clone();
        for guard in &sig.guards {
            self.expect(*guard, BaseSort::Bool.into(), context)?;
        }
        for arg in &sig.args {
            self.expect_entry(*arg, context)?;
        }
        context.extend(sig.existentials.iter().copied());
        for assert in &sig.asserts {
            self.expect(*assert, BaseSort::Bool.into(), context)?;
        }
        for result in &sig.results {
            self.expect_entry(*result, context)?;
        }
        Ok(())
    }

    /// The sort of `term` under `context` of bound variables.
    /// The last entry is `Bound(0)`.
    fn sort_under(
        &self,
        term: StaticTermId,
        context: &mut Vec<Sort<B>>,
    ) -> Result<Sort<B>, SortError<B>> {
        match &*self.terms[term] {
            StaticTerm::Bound(i) => {
                let Some(index) = context.len().checked_sub(i + 1) else {
                    return Err(SortError::UnboundVar {
                        term,
                        index: *i,
                        depth: context.len(),
                    });
                };

                Ok(context[index])
            }
            StaticTerm::Free(var) => Ok(self.vars[*var].sort),
            StaticTerm::Lit(v) => Ok(v.sort().into()),
            StaticTerm::Constructor(constructor, args) => {
                let sig = constructor.signature();

                // Check that the arguments have the expected sorts
                if args.len() != sig.args.len() {
                    return Err(SortError::ArityMismatch {
                        term,
                        shown: self.display(term, context.len()).to_string(),
                        expected: sig.args.len(),
                        found: args.len(),
                    });
                }
                for (arg, expected) in args.iter().zip(sig.args.iter()) {
                    self.expect(*arg, *expected, context)?;
                }
                Ok(sig.result.into())
            }
            StaticTerm::Seq(element, items) => {
                for item in items {
                    self.expect(*item, Sort::Base(*element), context)?;
                }
                Ok(Sort::Seq(*element))
            }
            StaticTerm::Guard(prop, ty) | StaticTerm::Assert(prop, ty) => {
                self.expect(*prop, BaseSort::Bool.into(), context)?;
                self.expect(*ty, BaseSort::Type.into(), context)?;
                Ok(BaseSort::Type.into())
            }
            StaticTerm::Forall(sort, body) | StaticTerm::Exists(sort, body) => {
                context.push(*sort);
                let checked = self.expect(*body, BaseSort::Type.into(), context);
                context.pop();
                checked?;
                Ok(BaseSort::Type.into())
            }
        }
    }

    /// Checks that `entry`, an argument or result of a [`TypeSig`], has sort
    /// `type` (one value) or `[type]` (a list of values), and that it contains
    /// no binder or proposition.
    fn expect_entry(
        &self,
        entry: StaticTermId,
        context: &mut Vec<Sort<B>>,
    ) -> Result<(), SortError<B>> {
        let found = self.sort_under(entry, context)?;
        if !matches!(
            found,
            Sort::Base(BaseSort::Type) | Sort::Seq(BaseSort::Type)
        ) {
            return Err(SortError::SortMismatch {
                term: entry,
                shown: self.display(entry, context.len()).to_string(),
                expected: BaseSort::Type.into(),
                found,
            });
        }

        match self.find_binder(entry) {
            // The outermost binder found has no binder above it in `entry`,
            // so it is displayed under `context` alone.
            Some(binder) => Err(SortError::NotPrenex {
                term: binder,
                shown: self.display(binder, context.len()).to_string(),
            }),
            None => Ok(()),
        }
    }

    /// The first `∀`, `∃`, `⊃` or `∧` in `term`, searching outermost first.
    fn find_binder(&self, term: StaticTermId) -> Option<StaticTermId> {
        use StaticTerm::*;
        match &*self.terms[term] {
            Forall(..) | Exists(..) | Guard(..) | Assert(..) => Some(term),
            Bound(_) | Free(_) | Lit(_) => None,
            Constructor(_, args) | Seq(_, args) => args.iter().find_map(|a| self.find_binder(*a)),
        }
    }

    /// Checks that `arg`, under all of `context`'s binders, has sort `expected`.
    fn expect(
        &self,
        arg: StaticTermId,
        expected: Sort<B>,
        context: &mut Vec<Sort<B>>,
    ) -> Result<(), SortError<B>> {
        let found = self.sort_under(arg, context)?;
        if found != expected {
            return Err(SortError::SortMismatch {
                term: arg,
                shown: self.display(arg, context.len()).to_string(),
                expected,
                found,
            });
        }
        Ok(())
    }
}
