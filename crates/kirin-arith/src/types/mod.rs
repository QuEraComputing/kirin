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
pub use arith_value::{ArithConversionError, ArithValue};

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

    #[test]
    fn test_arith_value_all_integer_type_mappings() {
        assert_eq!(ArithValue::I16(1).type_of(), ArithType::I16);
        assert_eq!(ArithValue::I32(1).type_of(), ArithType::I32);
        assert_eq!(ArithValue::U8(1).type_of(), ArithType::U8);
        assert_eq!(ArithValue::U16(1).type_of(), ArithType::U16);
        assert_eq!(ArithValue::U32(1).type_of(), ArithType::U32);
        assert_eq!(ArithValue::U128(1).type_of(), ArithType::U128);
        assert_eq!(ArithValue::F64(1.0).type_of(), ArithType::F64);
    }

    #[test]
    fn test_arith_value_display_integers() {
        assert_eq!(ArithValue::I64(42).to_string(), "42");
        assert_eq!(ArithValue::I64(-1).to_string(), "-1");
        assert_eq!(ArithValue::I64(0).to_string(), "0");
        assert_eq!(ArithValue::U64(100).to_string(), "100");
    }

    #[test]
    fn test_arith_value_display_float_edge_cases() {
        assert_eq!(ArithValue::F64(0.0).to_string(), "0.0");
        assert_eq!(ArithValue::F32(1.0).to_string(), "1.0");
        assert_eq!(ArithValue::F64(2.72).to_string(), "2.72");
    }

    #[test]
    fn test_parse_arith_type_all_variants() {
        assert_eq!(parse_ast::<ArithType>("i8").unwrap(), ArithType::I8);
        assert_eq!(parse_ast::<ArithType>("i16").unwrap(), ArithType::I16);
        assert_eq!(parse_ast::<ArithType>("i32").unwrap(), ArithType::I32);
        assert_eq!(parse_ast::<ArithType>("i64").unwrap(), ArithType::I64);
        assert_eq!(parse_ast::<ArithType>("i128").unwrap(), ArithType::I128);
        assert_eq!(parse_ast::<ArithType>("u8").unwrap(), ArithType::U8);
        assert_eq!(parse_ast::<ArithType>("u16").unwrap(), ArithType::U16);
        assert_eq!(parse_ast::<ArithType>("u32").unwrap(), ArithType::U32);
        assert_eq!(parse_ast::<ArithType>("u64").unwrap(), ArithType::U64);
        assert_eq!(parse_ast::<ArithType>("u128").unwrap(), ArithType::U128);
        assert_eq!(parse_ast::<ArithType>("f32").unwrap(), ArithType::F32);
        assert_eq!(parse_ast::<ArithType>("f64").unwrap(), ArithType::F64);
    }

    #[test]
    fn test_parse_arith_value_boundary_integers() {
        let max = format!("{}", i64::MAX);
        assert_eq!(
            parse_ast::<ArithValue>(&max).unwrap(),
            ArithValue::I64(i64::MAX)
        );

        let min = format!("{}", i64::MIN);
        assert_eq!(
            parse_ast::<ArithValue>(&min).unwrap(),
            ArithValue::I64(i64::MIN)
        );
    }

    // --- TryFrom<ArithValue> for i64 tests ---

    #[test]
    fn try_from_i64_is_infallible() {
        assert_eq!(i64::try_from(ArithValue::I64(42)), Ok(42));
        assert_eq!(i64::try_from(ArithValue::I64(i64::MAX)), Ok(i64::MAX));
        assert_eq!(i64::try_from(ArithValue::I64(i64::MIN)), Ok(i64::MIN));
    }

    #[test]
    fn try_from_widening_integer_is_infallible() {
        assert_eq!(i64::try_from(ArithValue::I8(127)), Ok(127));
        assert_eq!(i64::try_from(ArithValue::I16(-32768)), Ok(-32768));
        assert_eq!(
            i64::try_from(ArithValue::I32(i32::MAX as i64 as i32)),
            Ok(i32::MAX as i64)
        );
        assert_eq!(i64::try_from(ArithValue::U8(255)), Ok(255));
        assert_eq!(i64::try_from(ArithValue::U16(65535)), Ok(65535));
        assert_eq!(
            i64::try_from(ArithValue::U32(u32::MAX)),
            Ok(u32::MAX as i64)
        );
    }

    #[test]
    fn try_from_i128_out_of_range_is_err() {
        assert!(i64::try_from(ArithValue::I128(i128::MAX)).is_err());
        assert!(i64::try_from(ArithValue::I128(i128::MIN)).is_err());
    }

    #[test]
    fn try_from_i128_in_range_is_ok() {
        assert_eq!(i64::try_from(ArithValue::I128(42)), Ok(42));
        assert_eq!(
            i64::try_from(ArithValue::I128(i64::MAX as i128)),
            Ok(i64::MAX)
        );
    }

    #[test]
    fn try_from_u64_over_max_is_err() {
        assert!(i64::try_from(ArithValue::U64(u64::MAX)).is_err());
        assert!(i64::try_from(ArithValue::U64(i64::MAX as u64 + 1)).is_err());
    }

    #[test]
    fn try_from_u64_in_range_is_ok() {
        assert_eq!(i64::try_from(ArithValue::U64(0)), Ok(0));
        assert_eq!(
            i64::try_from(ArithValue::U64(i64::MAX as u64)),
            Ok(i64::MAX)
        );
    }

    #[test]
    fn try_from_u128_out_of_range_is_err() {
        assert!(i64::try_from(ArithValue::U128(u128::MAX)).is_err());
    }

    #[test]
    fn try_from_float_whole_is_ok() {
        assert_eq!(i64::try_from(ArithValue::F64(42.0)), Ok(42));
        assert_eq!(i64::try_from(ArithValue::F32(0.0)), Ok(0));
    }

    #[test]
    fn try_from_float_fractional_is_err() {
        assert!(i64::try_from(ArithValue::F64(2.5)).is_err());
        assert!(i64::try_from(ArithValue::F32(0.1)).is_err());
    }

    #[test]
    fn try_from_float_nonfinite_is_err() {
        assert!(i64::try_from(ArithValue::F64(f64::INFINITY)).is_err());
        assert!(i64::try_from(ArithValue::F64(f64::NAN)).is_err());
        assert!(i64::try_from(ArithValue::F32(f32::NEG_INFINITY)).is_err());
    }

    #[test]
    fn to_i64_lossy_preserves_old_behavior() {
        // Lossy wrapping for values outside i64 range.
        let _ = ArithValue::U64(u64::MAX).to_i64_lossy();
        let _ = ArithValue::I128(i128::MAX).to_i64_lossy();
        // Normal conversions.
        assert_eq!(ArithValue::I64(42).to_i64_lossy(), 42);
        assert_eq!(ArithValue::I8(-1).to_i64_lossy(), -1);
    }
}
