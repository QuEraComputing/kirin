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
