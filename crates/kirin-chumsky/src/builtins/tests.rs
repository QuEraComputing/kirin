use crate::traits::HasParser;

fn parse_with<T: HasParser<'static, 'static>>(input: &'static str) -> Result<T::Output, ()> {
    kirin_test_utils::parse_tokens!(input, T::parser()).map_err(|_| ())
}

#[test]
fn test_parse_i32() {
    assert_eq!(parse_with::<i32>("42"), Ok(42));
    assert_eq!(parse_with::<i32>("-123"), Ok(-123));
    assert_eq!(parse_with::<i32>("0"), Ok(0));
}

#[test]
fn test_parse_u32() {
    assert_eq!(parse_with::<u32>("42"), Ok(42));
    assert_eq!(parse_with::<u32>("0"), Ok(0));
    // Negative should fail for unsigned
    assert!(parse_with::<u32>("-1").is_err());
}

#[test]
fn test_parse_u64_hex() {
    assert_eq!(parse_with::<u64>("0xff"), Ok(255));
    assert_eq!(parse_with::<u64>("0xDEADBEEF"), Ok(0xDEADBEEF));
}

#[test]
fn test_parse_f64() {
    assert_eq!(parse_with::<f64>("3.14"), Ok(3.14));
    assert_eq!(parse_with::<f64>("1"), Ok(1.0));
    assert_eq!(parse_with::<f64>("-2.5"), Ok(-2.5));
}

#[test]
fn test_parse_bool() {
    assert_eq!(parse_with::<bool>("true"), Ok(true));
    assert_eq!(parse_with::<bool>("false"), Ok(false));
}

#[test]
fn test_parse_string() {
    assert_eq!(parse_with::<String>("hello"), Ok("hello".to_string()));
    assert_eq!(parse_with::<String>("\"quoted\""), Ok("quoted".to_string()));
}

// === Additional integer type tests ===

#[test]
fn test_parse_i8() {
    assert_eq!(parse_with::<i8>("127"), Ok(127));
    assert_eq!(parse_with::<i8>("-128"), Ok(-128));
    assert_eq!(parse_with::<i8>("0"), Ok(0));
}

#[test]
fn test_parse_i8_overflow() {
    assert!(parse_with::<i8>("128").is_err());
    assert!(parse_with::<i8>("-129").is_err());
}

#[test]
fn test_parse_i16() {
    assert_eq!(parse_with::<i16>("32767"), Ok(32767));
    assert_eq!(parse_with::<i16>("-32768"), Ok(-32768));
}

#[test]
fn test_parse_i64() {
    assert_eq!(parse_with::<i64>("9223372036854775807"), Ok(i64::MAX));
    assert_eq!(parse_with::<i64>("-1"), Ok(-1));
}

#[test]
fn test_parse_isize() {
    assert_eq!(parse_with::<isize>("42"), Ok(42));
    assert_eq!(parse_with::<isize>("-42"), Ok(-42));
}

#[test]
fn test_parse_u8() {
    assert_eq!(parse_with::<u8>("255"), Ok(255));
    assert_eq!(parse_with::<u8>("0"), Ok(0));
}

#[test]
fn test_parse_u8_overflow() {
    assert!(parse_with::<u8>("256").is_err());
}

#[test]
fn test_parse_u16() {
    assert_eq!(parse_with::<u16>("65535"), Ok(65535));
    assert_eq!(parse_with::<u16>("0"), Ok(0));
}

#[test]
fn test_parse_usize() {
    assert_eq!(parse_with::<usize>("12345"), Ok(12345));
}

#[test]
fn test_parse_f32() {
    assert_eq!(parse_with::<f32>("1.5"), Ok(1.5f32));
    assert_eq!(parse_with::<f32>("0"), Ok(0.0f32));
}

#[test]
fn test_parse_i32_overflow() {
    assert!(parse_with::<i32>("2147483648").is_err());
}

#[test]
fn test_parse_u32_overflow() {
    assert!(parse_with::<u32>("4294967296").is_err());
}
