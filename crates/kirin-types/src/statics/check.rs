use crate::SortError;

use super::{IndexSort, Sort, StaticSig, StaticTerm, StaticTermId, Statics};

impl<B: IndexSort, C: StaticSig<B>> Statics<B, C> {
    /// The sort of the locally closed `term`.
    /// Locally closed means that there are no `Bound(i)` variables without a
    /// corresponding enclosing binder.
    pub fn check_sort(&self, term: StaticTermId) -> Result<Sort<B>, SortError<B>> {
        self.sort_under(term, &mut Vec::new())
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
            StaticTerm::Lit(v) => Ok(v.sort()),
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
                Ok(sig.result)
            }
            StaticTerm::Unit => Ok(Sort::Type),
            StaticTerm::Product(components) => {
                if components.len() < 2 {
                    return Err(SortError::ShortProduct {
                        term,
                        shown: self.display(term, context.len()).to_string(),
                        found: components.len(),
                    });
                }
                for component in components {
                    self.expect(*component, Sort::Type, context)?;
                }
                Ok(Sort::Type)
            }
            StaticTerm::Function(args, result) => {
                for arg in args.iter().chain([result]) {
                    self.expect(*arg, Sort::Type, context)?;
                }
                Ok(Sort::Type)
            }
            StaticTerm::Guard(prop, ty) | StaticTerm::Assert(prop, ty) => {
                self.expect(*prop, Sort::Bool, context)?;
                self.expect(*ty, Sort::Type, context)?;
                Ok(Sort::Type)
            }
            StaticTerm::Forall(sort, body) | StaticTerm::Exists(sort, body) => {
                context.push(*sort);
                let checked = self.expect(*body, Sort::Type, context);
                context.pop();
                checked?;
                Ok(Sort::Type)
            }
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
