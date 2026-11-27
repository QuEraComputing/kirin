//! Constructors, sorts, and a statics fixture shared by the integration tests.
#![allow(dead_code)]

use std::fmt;

use kirin_types::{
    BaseSort, BoolStaticCon, IntSort, IntSortT, IntStaticCon, Sort, SortError, SortValue,
    StaticCon, StaticSig, StaticTerm, StaticTermId, Statics,
};

/// The `bool` and `int` constructors plus:
///  - `token : () ⇒ type`, a type with no structure to stand in wherever a
///    test needs some type,
///  - `Tuple : ([type]) ⇒ type`, a language-defined tuple type,
///  - `Tensor : (type, [int]) ⇒ type`, a type indexed by a list of ints.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TestCon {
    Bool(BoolStaticCon<IntSort>),
    Int(IntStaticCon),
    Token,
    Tuple,
    Tensor,
}

impl StaticSig<IntSort> for TestCon {
    fn signature(&self) -> StaticCon<IntSort> {
        match self {
            Self::Bool(con) => con.signature(),
            Self::Int(con) => con.signature(),
            Self::Token => StaticCon {
                args: vec![],
                result: BaseSort::Type,
            },
            Self::Tuple => StaticCon {
                args: vec![types()],
                result: BaseSort::Type,
            },
            Self::Tensor => StaticCon {
                args: vec![ty(), Sort::Seq(BaseSort::Index(IntSort))],
                result: BaseSort::Type,
            },
        }
    }

    fn eval(&self, args: &[SortValue<IntSortT>]) -> Option<SortValue<IntSortT>> {
        match self {
            Self::Bool(con) => con.eval(args),
            Self::Int(con) => StaticSig::<IntSort>::eval(con, args),
            Self::Token | Self::Tuple | Self::Tensor => None,
        }
    }
}

impl fmt::Display for TestCon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(con) => con.fmt(f),
            Self::Int(con) => con.fmt(f),
            Self::Token => f.write_str("token"),
            Self::Tuple => f.write_str("Tuple"),
            Self::Tensor => f.write_str("Tensor"),
        }
    }
}

pub type TestSort = Sort<IntSort>;
pub type TestTerm = StaticTerm<IntSort, TestCon>;

pub fn int() -> TestSort {
    Sort::Base(BaseSort::Index(IntSort))
}

pub fn boolean() -> TestSort {
    Sort::Base(BaseSort::Bool)
}

pub fn ty() -> TestSort {
    Sort::Base(BaseSort::Type)
}

/// `[type]`, the sort of a list of types.
pub fn types() -> TestSort {
    Sort::Seq(BaseSort::Type)
}

/// Statics plus shorthands for building terms.
pub struct Fixture {
    pub statics: Statics<IntSort, TestCon>,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            statics: Statics::default(),
        }
    }

    pub fn term(&mut self, term: TestTerm) -> StaticTermId {
        self.statics.alloc(term)
    }

    pub fn bound(&mut self, i: usize) -> StaticTermId {
        self.term(StaticTerm::Bound(i))
    }

    /// A fresh free variable of sort `sort`, as a term.
    pub fn free(&mut self, sort: TestSort) -> StaticTermId {
        let var = self.statics.fresh_var(sort, None);
        self.term(StaticTerm::Free(var))
    }

    pub fn int_con(&mut self, con: IntStaticCon, args: Vec<StaticTermId>) -> StaticTermId {
        self.term(StaticTerm::Constructor(TestCon::Int(con), args))
    }

    pub fn bool_con(
        &mut self,
        con: BoolStaticCon<IntSort>,
        args: Vec<StaticTermId>,
    ) -> StaticTermId {
        self.term(StaticTerm::Constructor(TestCon::Bool(con), args))
    }

    /// The `int` literal `n`.
    pub fn lit(&mut self, n: i64) -> StaticTermId {
        self.term(StaticTerm::Lit(SortValue::Index(IntSortT(n))))
    }

    /// The `bool` literal `b`.
    pub fn bool_lit(&mut self, b: bool) -> StaticTermId {
        self.term(StaticTerm::Lit(SortValue::Bool(b)))
    }

    /// The list of types `[items]`.
    pub fn types(&mut self, items: Vec<StaticTermId>) -> StaticTermId {
        self.term(StaticTerm::Seq(BaseSort::Type, items))
    }

    pub fn token(&mut self) -> StaticTermId {
        self.term(StaticTerm::Constructor(TestCon::Token, vec![]))
    }

    pub fn check(&self, term: StaticTermId) -> Result<TestSort, SortError<IntSort>> {
        self.statics.check_sort(term)
    }
}
