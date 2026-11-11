use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use kirin_ir::{Arena, identifier};

/// What a dialect's sort must implement.
/// Often implemented for enums that compose multiple [`IndexSort`] variants.
pub trait IndexSort: Copy + Eq + Hash + Debug {
    /// The underlying data type that an index sort evaluates to
    /// e.g. `IntSortValue(i64)` for the `int` sort.
    type T: Clone + Eq + Hash + Debug;

    /// If we only have the `value`, retrieve the sort that it belongs to.
    fn sort_of(value: &Self::T) -> Self;

    /// How the sort is written, e.g. `int`.
    fn name(&self) -> &'static str;
}

/// The statics are first-order: every sort is a base sort.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sort<S> {
    /// `type`: the sort of types of dynamic terms.
    Type,
    /// `bool`: the sort of static propositions, e.g. guards.
    /// Separate variant from `Index` since some [`StaticTerm`] variants rely
    /// on it.
    Bool,
    /// An index sort contributed by a dialect, e.g. `int`.
    Index(S),
}

/// Automtically convert an [`IndexSort`] to a [`Sort`]
impl<S: IndexSort> From<S> for Sort<S> {
    fn from(base: S) -> Self {
        Sort::Index(base)
    }
}

/// A dialect may compose many sorts, e.g. IntSort, FloatSort, etc.
/// When constraints combine multiple such sorts we need a way of extracting
/// the underlying values. This is provided by this trait.
pub trait Project<T> {
    fn project(&self) -> Option<&T>;
}

impl<T> Project<T> for T {
    fn project(&self) -> Option<&T> {
        Some(self)
    }
}

/// The underlying value of a [`Sort`]:
///   - a `bool` for [`Sort::Bool`],
///   - a value `V` for [`Sort::Index`]
///
///
/// [`Sort::Type`] has no underlying value
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortValue<V> {
    Bool(bool),
    Index(V),
}

impl<V> SortValue<V> {
    /// The `T` this value holds, if it is a value of `T`'s sort.
    pub fn project<T>(&self) -> Option<&T>
    where
        V: Project<T>,
    {
        match self {
            SortValue::Index(value) => value.project(),
            SortValue::Bool(_) => None,
        }
    }

    /// The boolean this value is, if it is a `bool`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            SortValue::Bool(b) => Some(*b),
            SortValue::Index(_) => None,
        }
    }

    /// The sort this value belongs to.
    pub fn sort<S: IndexSort<T = V>>(&self) -> Sort<S> {
        match self {
            SortValue::Bool(_) => Sort::Bool,
            SortValue::Index(value) => Sort::Index(S::sort_of(value)),
        }
    }
}

/// The constructor for a static term defined in the dialect.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StaticCon<S> {
    pub args: Vec<Sort<S>>,
    pub result: Sort<S>,
}

pub trait StaticSig<S: IndexSort>: Clone + PartialEq + Display {
    // API to define the static constructors in a dialect.
    fn signature(&self) -> StaticCon<S>;
    /// Evaluate a constructor when applied to `args`.
    /// Returns `None` if this constructor does not evaluate to a value
    /// (e.g. [`crate::IntStaticCon::IntOf`]).
    fn eval(&self, args: &[SortValue<S::T>]) -> Option<SortValue<S::T>>;
}

identifier! {
    /// A static term allocated in [`Statics::terms`].
    struct StaticTermId
}

identifier! {
    /// A free static variable allocated in [`Statics::vars`].
    struct StaticVarId
}

/// A free static variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticVar<S> {
    /// The sort of the variable.
    pub sort: Sort<S>,
    /// A name for diagnostics, e.g. from an annotation or the SSA value the
    /// variable was introduced for.
    pub name: Option<String>,
}

/// Terms are kept in an `Arena` in `Statics` so they can be shared.
/// The derived `PartialEq` is shallow: it compares child ids, not the terms
/// they point to. Use [`Statics::term_eq`] to compare terms.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StaticTerm<S: IndexSort, C> {
    /// A variable is either
    ///  - `Bound(i)`, a de Bruijn index: references the `i`-th innermost
    ///    binder around the variable, e.g. `Bound(0)` is the innermost.
    ///    Binders are [`StaticTerm::Forall`]/[`StaticTerm::Exists`] and the
    ///    quantifiers of a [`TypeSig`](crate::TypeSig).
    Bound(usize),
    ///  - `Free(x)`, a variable whose sort is recorded in [`Statics::vars`],
    ///    introduced by the type checker when it opens a binder, or by
    ///    whoever builds the term.
    Free(StaticVarId),
    /// A literal value: a `bool`, or a value of a dialect's base sort.
    Lit(SortValue<S::T>),
    /// A dialect's signature applied to its arguments.
    Constructor(C, Vec<StaticTermId>),
    /// The unit type for runtime values
    Unit,
    /// `T_1 * … * T_n` with `n ≥ 2`, of sort `type`.
    Product(Vec<StaticTermId>),
    /// `(T_1, …, T_n) → T`, of sort `type`.
    Function(Vec<StaticTermId>, StaticTermId),
    /// `B ⊃ T`: a guarded type, usable only where `B` holds.
    Guard(StaticTermId, StaticTermId),
    /// `B ∧ T`: an asserting type, whose values establish `B`.
    Assert(StaticTermId, StaticTermId),
    /// `∀a:σ. T`, binding `a` as `Bound(0)` in `T`.
    Forall(Sort<S>, StaticTermId),
    /// `∃a:σ. T`, binding `a` as `Bound(0)` in `T`.
    Exists(Sort<S>, StaticTermId),
}

/// The static terms of one program, and their free variables.
pub struct Statics<S: IndexSort, C> {
    pub terms: Arena<StaticTermId, StaticTerm<S, C>>,
    pub vars: Arena<StaticVarId, StaticVar<S>>,
}

impl<S: IndexSort, C> Default for Statics<S, C> {
    fn default() -> Self {
        Self {
            terms: Arena::default(),
            vars: Arena::default(),
        }
    }
}

impl<S: IndexSort, C> Statics<S, C> {
    pub fn alloc(&mut self, term: StaticTerm<S, C>) -> StaticTermId {
        self.terms.alloc(term)
    }

    /// A new free variable of sort `sort`, with an optional name for
    /// diagnostics.
    pub fn fresh_var(&mut self, sort: Sort<S>, name: Option<String>) -> StaticVarId {
        self.vars.alloc(StaticVar { sort, name })
    }

    /// Whether the free variable `var` occurs in `term`.
    pub fn mentions(&self, term: StaticTermId, var: StaticVarId) -> bool {
        use StaticTerm::*;
        match &*self.terms[term] {
            Free(x) => *x == var,
            Unit | Bound(_) | Lit(_) => false,
            Constructor(_, args) | Product(args) => args.iter().any(|a| self.mentions(*a, var)),
            Function(args, result) => args.iter().chain([result]).any(|a| self.mentions(*a, var)),
            Guard(p, t) | Assert(p, t) => self.mentions(*p, var) || self.mentions(*t, var),
            Forall(_, body) | Exists(_, body) => self.mentions(*body, var),
        }
    }
}

impl<S: IndexSort, C: PartialEq> Statics<S, C> {
    /// Whether `a` and `b` are the same term, possibly stored at different ids.
    /// Bound variables are de Bruijn indices, so terms that differ only in the
    /// names of bound variables are identical: this is a plain structural
    /// comparison.
    pub fn term_eq(&self, a: StaticTermId, b: StaticTermId) -> bool {
        use StaticTerm::*;
        let all_eq = |xs: &[StaticTermId], ys: &[StaticTermId]| {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| self.term_eq(*x, *y))
        };
        if a == b {
            return true;
        }
        match (&*self.terms[a], &*self.terms[b]) {
            (Unit, Unit) => true,
            (Bound(i), Bound(j)) => i == j,
            (Free(x), Free(y)) => x == y,
            (Lit(x), Lit(y)) => x == y,
            (Constructor(c, xs), Constructor(d, ys)) => c == d && all_eq(xs, ys),
            (Product(xs), Product(ys)) => all_eq(xs, ys),
            (Function(xs, r), Function(ys, s)) => all_eq(xs, ys) && self.term_eq(*r, *s),
            (Guard(p, t), Guard(q, u)) | (Assert(p, t), Assert(q, u)) => {
                self.term_eq(*p, *q) && self.term_eq(*t, *u)
            }
            (Forall(s, x), Forall(t, y)) | (Exists(s, x), Exists(t, y)) => {
                s == t && self.term_eq(*x, *y)
            }
            _ => false,
        }
    }
}

impl<S: IndexSort, C: Clone> Statics<S, C> {
    /// Maps `term` and its children nodes with function `f`. Traversal of
    /// the nodes is done bottom-up, such that a node's children are mapped
    /// before the node itself.
    /// The function `f` is called with the following arguments:
    ///  * `self`: the `Statics` object to allocate any new terms.
    ///  * `term`: the original version of the current node being visited.
    ///    This is the same `term` as is `map_term`'s arguments
    ///  * `&rebuilt`: the version of the current node but with the children
    ///    already mapped by `f`.
    ///  * `depth`: The current depth from the starting node.
    pub(super) fn map_term<E>(
        &mut self,
        term: StaticTermId,
        depth: usize,
        f: &impl Fn(
            &mut Self,
            StaticTermId,
            &StaticTerm<S, C>,
            usize,
        ) -> Result<Option<StaticTermId>, E>,
    ) -> Result<StaticTermId, E> {
        use StaticTerm::*;
        let mut changed = false;
        let mut map = |statics: &mut Self, child: StaticTermId, depth: usize| {
            let mapped = statics.map_term(child, depth, f)?;
            changed |= mapped != child;
            Ok(mapped)
        };
        // Recursively apply `map_term` to the `term`'s children.
        // We take a copy of the node, so the recursive `&mut self` calls don't
        // overlap its borrow.
        let rebuilt = match (*self.terms[term]).clone() {
            leaf @ (Unit | Bound(_) | Free(_) | Lit(_)) => leaf,
            Constructor(c, args) => Constructor(
                c,
                args.into_iter()
                    .map(|arg| map(self, arg, depth))
                    .collect::<Result<_, _>>()?,
            ),
            Product(args) => Product(
                args.into_iter()
                    .map(|arg| map(self, arg, depth))
                    .collect::<Result<_, _>>()?,
            ),
            Function(args, result) => {
                let args = args
                    .into_iter()
                    .map(|arg| map(self, arg, depth))
                    .collect::<Result<_, _>>()?;
                Function(args, map(self, result, depth)?)
            }
            Guard(p, t) => Guard(map(self, p, depth)?, map(self, t, depth)?),
            Assert(p, t) => Assert(map(self, p, depth)?, map(self, t, depth)?),
            Forall(sort, body) => Forall(sort, map(self, body, depth + 1)?),
            Exists(sort, body) => Exists(sort, map(self, body, depth + 1)?),
        };

        // Apply `f` to the node and return any changes made to the current node
        if let Some(replacement) = f(self, term, &rebuilt, depth)? {
            return Ok(replacement);
        }

        // If any changes were made to the children, return the rebuilt node
        Ok(if changed { self.alloc(rebuilt) } else { term })
    }
}
