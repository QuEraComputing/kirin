use kirin_ir::*;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum SimpleIRType {
    Any,
    Int,
    Float,
    DataType,
    Bottom,
}

pub use SimpleIRType::*;

impl Lattice for SimpleIRType {
    fn is_subseteq(&self, other: &Self) -> bool {
        matches!((self, other), (a, b) if a == b)
    }

    fn join(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            other.clone()
        } else if other.is_subseteq(self) {
            self.clone()
        } else {
            SimpleIRType::Any
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            self.clone()
        } else if other.is_subseteq(self) {
            other.clone()
        } else {
            SimpleIRType::Bottom
        }
    }
}

impl FiniteLattice for SimpleIRType {
    fn bottom() -> Self {
        SimpleIRType::Bottom
    }

    fn top() -> Self {
        SimpleIRType::Any
    }
}

impl Default for SimpleIRType {
    fn default() -> Self {
        Self::bottom()
    }
}

impl std::fmt::Display for SimpleIRType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleIRType::Any => write!(f, "any"),
            SimpleIRType::Int => write!(f, "int"),
            SimpleIRType::Float => write!(f, "float"),
            SimpleIRType::DataType => write!(f, "datatype"),
            SimpleIRType::Bottom => write!(f, "bottom"),
        }
    }
}

impl crate::TypeLattice for SimpleIRType {}

impl Typeof<SimpleIRType> for i64 {
    fn type_of(&self) -> SimpleIRType {
        SimpleIRType::Int
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I64(i64),
    F64(f64),
}
impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::I64(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            Value::F64(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
        }
    }
}

impl Typeof<SimpleIRType> for Value {
    fn type_of(&self) -> SimpleIRType {
        match self {
            Value::I64(_) => SimpleIRType::Int,
            Value::F64(_) => SimpleIRType::Float,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::I64(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::I64(v) => write!(f, "{}", v),
            Value::F64(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(fn, type = SimpleIRType, crate = kirin_ir)]
pub enum SimpleLanguage {
    Add(
        SSAValue,
        SSAValue,
        #[kirin(type = SimpleIRType::Float)] ResultValue,
    ),
    Constant(
        #[kirin(into)] Value,
        #[kirin(type = SimpleIRType::Float)] ResultValue,
    ),
    #[kirin(terminator)]
    Return(SSAValue),
    Function(Region, #[kirin(type = SimpleIRType::Float)] ResultValue),
}

// ============================================================================
// SimpleType - A type lattice with parser support (requires "parser" feature)
// ============================================================================

/// Simple type lattice used for parser integration tests.
///
/// This type has more concrete type variants (i32, i64, f32, f64, bool, unit)
/// compared to `SimpleIRType` which uses abstract categories (Int, Float, etc).
///
/// Enable the `parser` feature to get `HasParser` and `DirectlyParsable` implementations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimpleType {
    I32,
    I64,
    F32,
    F64,
    Bool,
    Unit,
}

impl Lattice for SimpleType {
    fn join(&self, other: &Self) -> Self {
        if self == other {
            self.clone()
        } else {
            SimpleType::Unit
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self == other {
            self.clone()
        } else {
            SimpleType::Unit
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        self == other || matches!(other, SimpleType::Unit)
    }
}

impl FiniteLattice for SimpleType {
    fn bottom() -> Self {
        SimpleType::Unit
    }

    fn top() -> Self {
        SimpleType::Unit
    }
}

impl TypeLattice for SimpleType {}

impl Default for SimpleType {
    fn default() -> Self {
        SimpleType::Unit
    }
}

impl std::fmt::Display for SimpleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleType::I32 => write!(f, "i32"),
            SimpleType::I64 => write!(f, "i64"),
            SimpleType::F32 => write!(f, "f32"),
            SimpleType::F64 => write!(f, "f64"),
            SimpleType::Bool => write!(f, "bool"),
            SimpleType::Unit => write!(f, "unit"),
        }
    }
}

// ============================================================================
// UnitType - A minimal type lattice for testing
// ============================================================================

/// A minimal type lattice with a single value, useful for testing dialects
/// that don't need type annotations.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Default)]
pub struct UnitType;

impl std::fmt::Display for UnitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "()")
    }
}

impl Lattice for UnitType {
    fn join(&self, _: &Self) -> Self {
        UnitType
    }

    fn meet(&self, _: &Self) -> Self {
        UnitType
    }

    fn is_subseteq(&self, _: &Self) -> bool {
        true
    }
}

impl FiniteLattice for UnitType {
    fn top() -> Self {
        UnitType
    }

    fn bottom() -> Self {
        UnitType
    }
}

impl TypeLattice for UnitType {}

#[cfg(feature = "parser")]
mod parser_impls {
    use super::{SimpleType, UnitType};
    use chumsky::prelude::*;
    use kirin_chumsky::{BoxedParser, DirectlyParsable, HasParser, TokenInput};
    use kirin_lexer::Token;
    use kirin_prettyless::{ArenaDoc, DocAllocator, Document, PrettyPrint};

    impl DirectlyParsable for SimpleType {}

    impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for SimpleType {
        type Output = SimpleType;

        fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
        where
            I: TokenInput<'tokens, 'src>,
        {
            select! {
                Token::Identifier("i32") => SimpleType::I32,
                Token::Identifier("i64") => SimpleType::I64,
                Token::Identifier("f32") => SimpleType::F32,
                Token::Identifier("f64") => SimpleType::F64,
                Token::Identifier("bool") => SimpleType::Bool,
                Token::Identifier("unit") => SimpleType::Unit,
            }
            .labelled("type")
            .boxed()
        }
    }

    impl DirectlyParsable for UnitType {}

    impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for UnitType {
        type Output = UnitType;

        fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
        where
            I: TokenInput<'tokens, 'src>,
        {
            // Parse "()" as UnitType
            just(Token::LParen)
                .ignore_then(just(Token::RParen))
                .to(UnitType)
                .boxed()
        }
    }

    impl PrettyPrint for UnitType {
        fn pretty_print<'a, L: kirin_ir::Dialect + PrettyPrint>(
            &self,
            doc: &'a Document<'a, L>,
        ) -> ArenaDoc<'a>
        where
            L::Type: std::fmt::Display,
        {
            doc.text("()")
        }
    }
}
