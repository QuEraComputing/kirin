use kirin::parsers::HasParser;
use kirin::parsers::PrettyPrint;

/// Built-in arithmetic type lattice for `kirin-arith`.
///
/// This enum mirrors Rust primitive numeric type names in textual form (`i32`,
/// `u64`, `f64`, ...), making round-trip parse/print straightforward.
///
/// # Usage
///
/// ```rust,ignore
/// use kirin::parsers::parse_ast;
/// use kirin_arith::ArithType;
///
/// let ty = parse_ast::<ArithType>("i32").unwrap();
/// assert_eq!(ty, ArithType::I32);
/// assert_eq!(ty.to_string(), "i32");
/// ```
///
/// If this built-in lattice is not sufficient, define your own type enum and
/// use `Arith<YourType>` to preserve your language semantics.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, HasParser, PrettyPrint)]
pub enum ArithType {
    #[chumsky(format = "i8")]
    I8,
    #[chumsky(format = "i16")]
    I16,
    #[chumsky(format = "i32")]
    I32,
    #[chumsky(format = "i64")]
    I64,
    #[chumsky(format = "i128")]
    I128,
    #[chumsky(format = "u8")]
    U8,
    #[chumsky(format = "u16")]
    U16,
    #[chumsky(format = "u32")]
    U32,
    #[chumsky(format = "u64")]
    U64,
    #[chumsky(format = "u128")]
    U128,
    #[chumsky(format = "f32")]
    F32,
    #[chumsky(format = "f64")]
    F64,
}

#[allow(clippy::derivable_impls)]
impl Default for ArithType {
    fn default() -> Self {
        Self::I64
    }
}

impl kirin::ir::Placeholder for ArithType {
    fn placeholder() -> Self {
        Self::I64
    }
}
