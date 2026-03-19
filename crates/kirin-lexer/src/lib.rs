pub use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\r\f]+")]
#[logos(skip(r"//[^\n\r]*", allow_greedy = true))]
#[logos(skip r"/\*([^*]|\*+[^*/])*\*+/")]
pub enum Token<'src> {
    Error,
    /// ```ignore
    /// %<identifier>
    /// ```
    #[regex(r"%[\p{XID_Continue}_]+", |lex| &lex.slice()[1..])]
    SSAValue(&'src str),
    #[regex(r"\^[\p{XID_Continue}_]+", |lex| &lex.slice()[1..])]
    /// ```ignore
    /// ^<identifier>
    /// ```
    Block(&'src str),
    /// ```ignore
    /// <identifier>
    /// ```
    #[regex(r"[\p{XID_Start}_][\p{XID_Continue}_]*")]
    Identifier(&'src str),
    /// ```ignore
    /// @<symbol>
    /// ```
    #[regex(r"@[\p{XID_Continue}_]+", |lex| &lex.slice()[1..])]
    Symbol(&'src str),
    /// ```ignore
    /// #<attr_id>
    /// ```
    #[regex(r"#[\p{XID_Continue}_]+", |lex| &lex.slice()[1..])]
    AttrId(&'src str),

    #[regex(r"-?[0-9]+", |lex| lex.slice())]
    Int(&'src str),
    #[regex(r"0x[0-9a-fA-F]+", |lex| &lex.slice()[2..])]
    Unsigned(&'src str),
    #[regex(r"-?[0-9]+\.[0-9]+([eE]-?[0-9]+)?")]
    Float(&'src str),
    // Simple quoted string handling
    #[regex(r#""([^"\\]|\\["\\bnfrt]|u[a-fA-F0-9]{4})*""#, |lex| lex.slice().to_string())]
    StringLit(String),

    // --- Delimiters & Punctuation ---
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("{{")]
    EscapedLBrace,
    #[token("}")]
    RBrace,
    #[token("}}")]
    EscapedRBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("<")]
    LAngle,
    #[token(">")]
    RAngle,

    #[token("$")]
    Dollar,
    #[token("*")]
    Asterisk,
    #[token("?")]
    QuestionMark,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("=")]
    Equal,
    #[token("->")]
    Arrow,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token("...")]
    Ellipsis,
    #[token("::")]
    DoubleColon,
    #[token(";")]
    Semicolon,
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Error => write!(f, "error"),
            Token::SSAValue(name) => write!(f, "%{}", name),
            Token::Block(name) => write!(f, "^{}", name),
            Token::Identifier(name) => write!(f, "{}", name),
            Token::Symbol(name) => write!(f, "@{}", name),
            Token::AttrId(name) => write!(f, "#{}", name),
            Token::Int(value) => write!(f, "{}", value),
            Token::Unsigned(value) => write!(f, "0x{}", value),
            Token::Float(value) => write!(f, "{}", value),
            Token::StringLit(value) => write!(f, "{:?}", value),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::EscapedLBrace => write!(f, "{{{{"),
            Token::RBrace => write!(f, "}}"),
            Token::EscapedRBrace => write!(f, "}}}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LAngle => write!(f, "<"),
            Token::RAngle => write!(f, ">"),
            Token::Dollar => write!(f, "$"),
            Token::Asterisk => write!(f, "*"),
            Token::QuestionMark => write!(f, "?"),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Equal => write!(f, "="),
            Token::Arrow => write!(f, "->"),
            Token::Dot => write!(f, "."),
            Token::DotDot => write!(f, ".."),
            Token::Ellipsis => write!(f, "..."),
            Token::DoubleColon => write!(f, "::"),
            Token::Semicolon => write!(f, ";"),
        }
    }
}

pub fn lex<'src>(input: &'src str) -> impl Iterator<Item = Result<Token<'src>, String>> + 'src {
    Token::lexer(input)
        .spanned()
        .map(|(token, span)| match token {
            Ok(Token::Error) | Err(_) => {
                let text = &input[span.start..span.end.min(input.len())];
                Err(format!(
                    "Unexpected token '{}' at position {}",
                    text, span.start
                ))
            }
            Ok(t) => Ok(t),
        })
}

#[cfg(feature = "quote")]
impl quote::ToTokens for Token<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Token::Error => {
                tokens.extend(quote::quote! { compile_error!("Unexpected token") });
            }
            Token::SSAValue(name) => {
                tokens.extend(quote::quote! { Token::SSAValue(#name) });
            }
            Token::Block(name) => {
                tokens.extend(quote::quote! { Token::Block(#name) });
            }
            Token::Identifier(name) => {
                tokens.extend(quote::quote! { Token::Identifier(#name) });
            }
            Token::Symbol(name) => {
                tokens.extend(quote::quote! { Token::Symbol(#name) });
            }
            Token::AttrId(name) => {
                tokens.extend(quote::quote! { Token::AttrId(#name) });
            }
            Token::Int(value) => {
                tokens.extend(quote::quote! { Token::Int(#value) });
            }
            Token::Unsigned(value) => {
                tokens.extend(quote::quote! { Token::Unsigned(#value) });
            }
            Token::Float(value) => {
                tokens.extend(quote::quote! { Token::Float(#value) });
            }
            Token::StringLit(value) => {
                tokens.extend(quote::quote! { Token::StringLit(#value.to_string()) });
            }
            Token::LParen => {
                tokens.extend(quote::quote! { Token::LParen });
            }
            Token::RParen => {
                tokens.extend(quote::quote! { Token::RParen });
            }
            Token::LBrace => {
                tokens.extend(quote::quote! { Token::LBrace });
            }
            Token::EscapedLBrace => {
                tokens.extend(quote::quote! { Token::EscapedLBrace });
            }
            Token::RBrace => {
                tokens.extend(quote::quote! { Token::RBrace });
            }
            Token::EscapedRBrace => {
                tokens.extend(quote::quote! { Token::EscapedRBrace });
            }
            Token::LBracket => {
                tokens.extend(quote::quote! { Token::LBracket });
            }
            Token::RBracket => {
                tokens.extend(quote::quote! { Token::RBracket });
            }
            Token::LAngle => {
                tokens.extend(quote::quote! { Token::LAngle });
            }
            Token::RAngle => {
                tokens.extend(quote::quote! { Token::RAngle });
            }
            Token::Dollar => {
                tokens.extend(quote::quote! { Token::Dollar });
            }
            Token::Asterisk => {
                tokens.extend(quote::quote! { Token::Asterisk });
            }
            Token::QuestionMark => {
                tokens.extend(quote::quote! { Token::QuestionMark });
            }
            Token::Colon => {
                tokens.extend(quote::quote! { Token::Colon });
            }
            Token::Comma => {
                tokens.extend(quote::quote! { Token::Comma });
            }
            Token::Equal => {
                tokens.extend(quote::quote! { Token::Equal });
            }
            Token::Arrow => {
                tokens.extend(quote::quote! { Token::Arrow });
            }
            Token::Dot => {
                tokens.extend(quote::quote! { Token::Dot });
            }
            Token::DotDot => {
                tokens.extend(quote::quote! { Token::DotDot });
            }
            Token::Ellipsis => {
                tokens.extend(quote::quote! { Token::Ellipsis });
            }
            Token::DoubleColon => {
                tokens.extend(quote::quote! { Token::DoubleColon });
            }
            Token::Semicolon => {
                tokens.extend(quote::quote! { Token::Semicolon });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_common() {
        let input = "%x, %y: i32 -> i32";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("x"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("y"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), None);

        let input = "%x = addi %y, 42";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("x"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Equal)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("addi"))));
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("y"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("42"))));
        assert_eq!(lexer.next(), None);

        let input = "fn example(%arg0: i32) -> i32 { return %arg0; }";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("fn"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("example"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("arg0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("return"))));
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("arg0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Semicolon)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);

        let input = "^bb0(%arg1: i32, %arg2: f64)";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Block("bb0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("arg1"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("arg2"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("f64"))));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_type_expr() {
        let input = "ptr<i32>";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("ptr"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LAngle)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::RAngle)));
        assert_eq!(lexer.next(), None);

        let input = "array<4, 10, f64>";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("array"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LAngle)));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("4"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("10"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("f64"))));
        assert_eq!(lexer.next(), Some(Ok(Token::RAngle)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_tensor() {
        let input = "tensor<4 x 4 x i32>";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("tensor"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LAngle)));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("4"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("4"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::RAngle)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_comments_are_skipped() {
        let input = "foo // line comment\n/* block\ncomment */ bar";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("foo"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("bar"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_block_comments_with_trailing_stars_are_skipped() {
        let input = "foo /*a**/ /** text **/ bar";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("foo"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("bar"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_dot_token() {
        let input = "arith.add";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("arith"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Dot)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("add"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_dot_dot_still_works() {
        let input = "a..b";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("a"))));
        assert_eq!(lexer.next(), Some(Ok(Token::DotDot)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("b"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_symbol() {
        let input = "@main @foo_bar";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Symbol("main"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Symbol("foo_bar"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_attr_id() {
        let input = "#attr #my_tag";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::AttrId("attr"))));
        assert_eq!(lexer.next(), Some(Ok(Token::AttrId("my_tag"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_unsigned_hex() {
        let input = "0xFF 0x00 0xDEAD";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Unsigned("FF"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Unsigned("00"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Unsigned("DEAD"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_float() {
        let input = "3.14 1.0 0.5";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Float("3.14"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Float("1.0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Float("0.5"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_float_scientific_notation() {
        let input = "1.0e3 2.5e-2";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Float("1.0e3"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Float("2.5e-2"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_string_lit() {
        let input = r#""hello" "world""#;
        let mut lexer = Token::lexer(input);
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::StringLit("\"hello\"".to_string())))
        );
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::StringLit("\"world\"".to_string())))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_escaped_braces() {
        let input = "{{ }}";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::EscapedLBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::EscapedRBrace)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_brackets() {
        let input = "[0, 1]";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::LBracket)));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("1"))));
        assert_eq!(lexer.next(), Some(Ok(Token::RBracket)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_dollar_asterisk_question() {
        let input = "$ * ?";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Dollar)));
        assert_eq!(lexer.next(), Some(Ok(Token::Asterisk)));
        assert_eq!(lexer.next(), Some(Ok(Token::QuestionMark)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_double_colon() {
        let input = "foo::bar";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("foo"))));
        assert_eq!(lexer.next(), Some(Ok(Token::DoubleColon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("bar"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_colon_vs_double_colon() {
        let input = ": ::";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::DoubleColon)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_ellipsis() {
        let input = "a...b";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("a"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Ellipsis)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("b"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_negative_integer() {
        let input = "-42 -1";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Int("-42"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("-1"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_negative_float() {
        let input = "-3.14 -0.5e-1";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Float("-3.14"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Float("-0.5e-1"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_lex_public_api() {
        let input = "%x = addi %y, 42";
        let tokens: Vec<_> = lex(input).collect();
        assert_eq!(
            tokens,
            vec![
                Ok(Token::SSAValue("x")),
                Ok(Token::Equal),
                Ok(Token::Identifier("addi")),
                Ok(Token::SSAValue("y")),
                Ok(Token::Comma),
                Ok(Token::Int("42")),
            ]
        );
    }

    #[test]
    fn test_lex_error_on_invalid_input() {
        // `~` is not a recognized token
        let tokens: Vec<_> = lex("~").collect();
        assert_eq!(tokens.len(), 1);
        assert!(tokens[0].is_err());
    }

    #[test]
    fn test_lex_empty_input() {
        let tokens: Vec<_> = lex("").collect();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_lex_whitespace_only() {
        let tokens: Vec<_> = lex("   \t\n  ").collect();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_unicode_identifier() {
        let input = "_foo α";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("_foo"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("α"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_display_roundtrip() {
        // Verify Display outputs expected strings for all token variants
        assert_eq!(Token::Error.to_string(), "error");
        assert_eq!(Token::SSAValue("x").to_string(), "%x");
        assert_eq!(Token::Block("bb0").to_string(), "^bb0");
        assert_eq!(Token::Identifier("foo").to_string(), "foo");
        assert_eq!(Token::Symbol("main").to_string(), "@main");
        assert_eq!(Token::AttrId("tag").to_string(), "#tag");
        assert_eq!(Token::Int("42").to_string(), "42");
        assert_eq!(Token::Unsigned("FF").to_string(), "0xFF");
        assert_eq!(Token::Float("3.14").to_string(), "3.14");
        assert_eq!(Token::LParen.to_string(), "(");
        assert_eq!(Token::RParen.to_string(), ")");
        // DESIGN NOTE: LBrace Display outputs `{` (the `{{` is Rust fmt escaping).
        // EscapedLBrace Display outputs `{{` (the `{{{{` is Rust fmt escaping).
        assert_eq!(Token::LBrace.to_string(), "{");
        assert_eq!(Token::RBrace.to_string(), "}");
        assert_eq!(Token::EscapedLBrace.to_string(), "{{");
        assert_eq!(Token::EscapedRBrace.to_string(), "}}");
        assert_eq!(Token::LBracket.to_string(), "[");
        assert_eq!(Token::RBracket.to_string(), "]");
        assert_eq!(Token::LAngle.to_string(), "<");
        assert_eq!(Token::RAngle.to_string(), ">");
        assert_eq!(Token::Dollar.to_string(), "$");
        assert_eq!(Token::Asterisk.to_string(), "*");
        assert_eq!(Token::QuestionMark.to_string(), "?");
        assert_eq!(Token::Colon.to_string(), ":");
        assert_eq!(Token::Comma.to_string(), ",");
        assert_eq!(Token::Equal.to_string(), "=");
        assert_eq!(Token::Arrow.to_string(), "->");
        assert_eq!(Token::Dot.to_string(), ".");
        assert_eq!(Token::DotDot.to_string(), "..");
        assert_eq!(Token::Ellipsis.to_string(), "...");
        assert_eq!(Token::DoubleColon.to_string(), "::");
        assert_eq!(Token::Semicolon.to_string(), ";");
    }

    // --- Integer literal edge cases ---

    #[test]
    fn test_int_boundary_values() {
        // i64::MAX as string
        let input = "9223372036854775807";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Int("9223372036854775807"))));
        assert_eq!(lexer.next(), None);

        // i64::MIN as string
        let input = "-9223372036854775808";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Int("-9223372036854775808"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_int_overflow_still_lexes_as_string() {
        // Larger than i64::MAX — lexer stores raw text, so this is fine
        let input = "99999999999999999999999";
        let mut lexer = Token::lexer(input);
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Int("99999999999999999999999")))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_int_zero() {
        let input = "0 -0";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Int("0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("-0"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_int_leading_zeros() {
        let input = "007 -007";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Int("007"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("-007"))));
        assert_eq!(lexer.next(), None);
    }

    // --- Float literal edge cases ---

    #[test]
    fn test_float_very_small() {
        let input = "0.0000000001";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Float("0.0000000001"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_float_very_large() {
        let input = "99999999999999.99999999999";
        let mut lexer = Token::lexer(input);
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::Float("99999999999999.99999999999")))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_float_large_exponent() {
        let input = "1.0e308 1.0e-308";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Float("1.0e308"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Float("1.0e-308"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_float_capital_e_exponent() {
        let input = "1.0E10 2.5E-3";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Float("1.0E10"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Float("2.5E-3"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_float_zero() {
        let input = "0.0 -0.0";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Float("0.0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Float("-0.0"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_no_trailing_dot_float() {
        // "1." should NOT be a Float — it should be Int + Dot
        let input = "1.";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Int("1"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Dot)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_no_leading_dot_float() {
        // ".5" should NOT be a Float — Dot + Int
        let input = ".5";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Dot)));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("5"))));
        assert_eq!(lexer.next(), None);
    }

    // --- Hex / Unsigned edge cases ---

    #[test]
    fn test_unsigned_hex_full_range() {
        let input = "0xFFFFFFFFFFFFFFFF";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Unsigned("FFFFFFFFFFFFFFFF"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_unsigned_hex_mixed_case() {
        let input = "0xAbCd01";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Unsigned("AbCd01"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_unsigned_hex_zero() {
        let input = "0x0";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Unsigned("0"))));
        assert_eq!(lexer.next(), None);
    }

    // --- String literal edge cases ---

    #[test]
    fn test_string_empty() {
        let input = r#""""#;
        let mut lexer = Token::lexer(input);
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::StringLit(r#""""#.to_string())))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_string_with_escapes() {
        let input = r#""\n\t\\\"""#;
        let mut lexer = Token::lexer(input);
        assert_eq!(
            lexer.next(),
            Some(Ok(Token::StringLit(r#""\n\t\\\"""#.to_string())))
        );
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_string_unclosed_is_error() {
        // An unclosed string should not produce a StringLit token
        let input = r#""hello"#;
        let mut lexer = Token::lexer(input);
        let tok = lexer.next();
        assert!(tok.is_some());
        assert!(tok.unwrap().is_err());
    }

    // --- Identifier edge cases ---

    #[test]
    fn test_identifier_underscore_only() {
        let input = "_";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("_"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_identifier_leading_underscores() {
        let input = "___foo __";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("___foo"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("__"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_identifier_with_digits() {
        let input = "x0 a123b _9";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("a123b"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("_9"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_identifier_unicode_extended() {
        // CJK ideograph and accented letter
        let input = "日本語 café";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("日本語"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("café"))));
        assert_eq!(lexer.next(), None);
    }

    // --- SSA / Block edge cases ---

    #[test]
    fn test_ssa_with_underscores_and_digits() {
        let input = "%_0 %arg_1_2 %_";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("_0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("arg_1_2"))));
        assert_eq!(lexer.next(), Some(Ok(Token::SSAValue("_"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_block_with_digits() {
        let input = "^bb0 ^entry ^_hidden";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Block("bb0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Block("entry"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Block("_hidden"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_bare_percent_is_error() {
        // A bare `%` followed by space is not a valid SSAValue
        let tokens: Vec<_> = lex("% x").collect();
        assert!(tokens[0].is_err());
    }

    #[test]
    fn test_bare_caret_is_error() {
        // A bare `^` followed by space is not a valid Block
        let tokens: Vec<_> = lex("^ bb").collect();
        assert!(tokens[0].is_err());
    }

    #[test]
    fn test_bare_at_is_error() {
        // A bare `@` followed by space is not a valid Symbol
        let tokens: Vec<_> = lex("@ foo").collect();
        assert!(tokens[0].is_err());
    }

    #[test]
    fn test_bare_hash_is_error() {
        // A bare `#` followed by space is not a valid AttrId
        let tokens: Vec<_> = lex("# tag").collect();
        assert!(tokens[0].is_err());
    }

    // --- Error token handling ---

    #[test]
    fn test_multiple_error_tokens() {
        let tokens: Vec<_> = lex("~ ! `").collect();
        assert_eq!(tokens.len(), 3);
        for tok in &tokens {
            assert!(tok.is_err());
        }
    }

    #[test]
    fn test_error_interspersed_with_valid() {
        let tokens: Vec<_> = lex("foo ~ bar").collect();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Ok(Token::Identifier("foo")));
        assert!(tokens[1].is_err());
        assert_eq!(tokens[2], Ok(Token::Identifier("bar")));
    }

    // --- Comment edge cases ---

    #[test]
    fn test_comment_only_input() {
        let tokens: Vec<_> = lex("// just a comment").collect();
        assert!(tokens.is_empty());

        let tokens: Vec<_> = lex("/* block only */").collect();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_line_comment_no_trailing_newline() {
        let input = "foo // trailing";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("foo"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_empty_block_comment() {
        let input = "a /**/ b";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("a"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("b"))));
        assert_eq!(lexer.next(), None);
    }

    // --- Punctuation ambiguity edge cases ---

    #[test]
    fn test_arrow_vs_negative_int() {
        // "->" should be Arrow, not negative-something
        let input = "-> -1";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Int("-1"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_dot_disambiguation() {
        // `.` vs `..` vs `...`
        let input = ". .. ...";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Dot)));
        assert_eq!(lexer.next(), Some(Ok(Token::DotDot)));
        assert_eq!(lexer.next(), Some(Ok(Token::Ellipsis)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_four_dots() {
        // Four dots: `...` + `.`
        let input = "....";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Ellipsis)));
        assert_eq!(lexer.next(), Some(Ok(Token::Dot)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_colon_disambiguation() {
        // `:` vs `::`
        let input = ": :: :";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::DoubleColon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), None);
    }

    // --- Display for StringLit ---

    #[test]
    fn test_display_string_lit_uses_debug_format() {
        // StringLit Display uses {:?} which adds quotes and escapes
        let tok = Token::StringLit("hello\nworld".to_string());
        let display = tok.to_string();
        assert_eq!(display, "\"hello\\nworld\"");
    }

    // --- Whitespace handling ---

    #[test]
    fn test_form_feed_is_skipped() {
        let input = "a\x0Cb";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("a"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("b"))));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_carriage_return_is_skipped() {
        // \r is in the skip pattern, so it's treated as whitespace
        let tokens: Vec<_> = lex("a\rb").collect();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Ok(Token::Identifier("a")));
        assert_eq!(tokens[1], Ok(Token::Identifier("b")));
    }
}
