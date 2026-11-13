//! Rendering sorts and static terms for diagnostics.

use std::fmt::{self, Display};

use kirin_ir::Id;

use crate::{IndexSort, Sort, SortValue, StaticTerm, StaticTermId, Statics};

impl<B: IndexSort> Display for Sort<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Type => "type",
            Self::Bool => "bool",
            Self::Index(base) => base.name(),
        })
    }
}

impl<V: Display> Display for SortValue<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => value.fmt(f),
            Self::Index(value) => value.fmt(f),
        }
    }
}

impl<B: IndexSort, C: Display> Statics<B, C> {
    /// Renders `term`, which sits under `depth` binders. Bound variables are
    /// named by de Bruijn level, so `a0` is the outermost binder; a free
    /// variable by its name hint, or else `x` followed by its id.
    pub fn display(&self, term: StaticTermId, depth: usize) -> impl Display + '_ {
        TermDisplay {
            statics: self,
            term,
            depth,
            nested: false,
        }
    }
}

struct TermDisplay<'a, B: IndexSort, C> {
    statics: &'a Statics<B, C>,
    term: StaticTermId,
    depth: usize,
    /// Parenthesize forms that would otherwise be ambiguous inside another term.
    nested: bool,
}

impl<B: IndexSort, C: Display> TermDisplay<'_, B, C> {
    fn child(&self, term: StaticTermId, depth: usize) -> Self {
        Self {
            statics: self.statics,
            term,
            depth,
            nested: true,
        }
    }

    fn list(&self, f: &mut fmt::Formatter<'_>, terms: &[StaticTermId], sep: &str) -> fmt::Result {
        for (i, term) in terms.iter().enumerate() {
            if i > 0 {
                f.write_str(sep)?;
            }
            self.child(*term, self.depth).fmt(f)?;
        }
        Ok(())
    }
}

impl<B: IndexSort, C: Display> Display for TermDisplay<'_, B, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let term = &*self.statics.terms[self.term];
        let compound = matches!(
            term,
            StaticTerm::Function(..)
                | StaticTerm::Guard(..)
                | StaticTerm::Assert(..)
                | StaticTerm::Forall(..)
                | StaticTerm::Exists(..)
        );
        if self.nested && compound {
            f.write_str("(")?;
        }
        match term {
            StaticTerm::Bound(i) => match self.depth.checked_sub(i + 1) {
                Some(level) => write!(f, "a{level}")?,
                None => write!(f, "?{i}")?,
            },
            StaticTerm::Free(var) => match &self.statics.vars[*var].name {
                Some(name) => f.write_str(name)?,
                None => write!(f, "x{}", Id::from(*var).raw())?,
            },
            StaticTerm::Lit(value) => write!(f, "{value:?}")?,
            StaticTerm::Constructor(constructor, args) if args.is_empty() => {
                write!(f, "{constructor}")?
            }
            StaticTerm::Constructor(constructor, args) => {
                write!(f, "{constructor}(")?;
                self.list(f, args, ", ")?;
                f.write_str(")")?;
            }
            StaticTerm::Unit => write!(f, "unit")?,
            StaticTerm::Product(components) => {
                f.write_str("(")?;
                self.list(f, components, " * ")?;
                f.write_str(")")?;
            }
            StaticTerm::Function(args, result) => {
                f.write_str("(")?;
                self.list(f, args, ", ")?;
                write!(f, ") → {}", self.child(*result, self.depth))?;
            }
            StaticTerm::Guard(prop, ty) => write!(
                f,
                "{} ⊃ {}",
                self.child(*prop, self.depth),
                self.child(*ty, self.depth)
            )?,
            StaticTerm::Assert(prop, ty) => write!(
                f,
                "{} ∧ {}",
                self.child(*prop, self.depth),
                self.child(*ty, self.depth)
            )?,
            StaticTerm::Forall(sort, body) | StaticTerm::Exists(sort, body) => {
                let quantifier = if matches!(term, StaticTerm::Forall(..)) {
                    '∀'
                } else {
                    '∃'
                };
                write!(
                    f,
                    "{quantifier}a{}: {sort}. {}",
                    self.depth,
                    self.child(*body, self.depth + 1)
                )?;
            }
        }
        if self.nested && compound {
            f.write_str(")")?;
        }
        Ok(())
    }
}
