//! Built-in numeric type/value pair for `kirin-arith`.
//!
//! `ArithType` and `ArithValue` mirror Rust primitive numeric categories and
//! provide a ready-to-use default for arithmetic-heavy languages.
//!
//! If your language needs different numeric behavior, define your own type and
//! compile-time value enums and use `Arith<YourType>` instead of `ArithType`.
//! This keeps arithmetic operations reusable while allowing language-specific
//! semantics.

mod arith_type;
mod arith_value;

pub use arith_type::ArithType;
pub use arith_value::ArithValue;

#[cfg(test)]
mod tests {
    use super::*;
    use kirin::ir::Typeof;
    use kirin::parsers::parse_ast;

    #[test]
    fn test_parse_arith_type() {
        assert_eq!(parse_ast::<ArithType>("i8").unwrap(), ArithType::I8);
        assert_eq!(parse_ast::<ArithType>("i64").unwrap(), ArithType::I64);
        assert_eq!(parse_ast::<ArithType>("u32").unwrap(), ArithType::U32);
        assert_eq!(parse_ast::<ArithType>("f64").unwrap(), ArithType::F64);
    }

    #[test]
    fn test_parse_arith_value_heuristics() {
        assert_eq!(parse_ast::<ArithValue>("42").unwrap(), ArithValue::I64(42));
        assert_eq!(parse_ast::<ArithValue>("-5").unwrap(), ArithValue::I64(-5));
        assert_eq!(
            parse_ast::<ArithValue>("3.25").unwrap(),
            ArithValue::F64(3.25)
        );
    }

    #[test]
    fn test_arith_value_type_mapping() {
        assert_eq!(ArithValue::I8(1).type_of(), ArithType::I8);
        assert_eq!(ArithValue::I128(2).type_of(), ArithType::I128);
        assert_eq!(ArithValue::U64(3).type_of(), ArithType::U64);
        assert_eq!(ArithValue::F32(4.0).type_of(), ArithType::F32);
    }

    #[test]
    fn test_arith_value_display_for_floats() {
        assert_eq!(ArithValue::F32(2.0).to_string(), "2.0");
        assert_eq!(ArithValue::F64(2.5).to_string(), "2.5");
    }
}
