pub use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\f]+")]
#[logos(skip r"//[^\n\r]*")]
#[logos(skip r"/\*([^*]|\*+[^*/])*\*+/")]
pub enum Token<'src> {
    Error,
    /// ```ignore
    /// %<identifier>
    /// ```
    #[regex(r"%[\p{XID_Continue}_$.]+", |lex| &lex.slice()[1..])]
    SSAValue(&'src str),
    #[regex(r"\^[\p{XID_Continue}_$.]+", |lex| &lex.slice()[1..])]
    /// ```ignore
    /// ^<identifier>
    /// ```
    Block(&'src str),
    /// ```ignore
    /// <identifier>
    /// ```
    #[regex(r"[\p{XID_Start}_][\p{XID_Continue}_$.]*")]
    Identifier(&'src str),
    /// ```ignore
    /// @<symbol>
    /// ```
    #[regex(r"@[\p{XID_Continue}_$.]+", |lex| &lex.slice()[1..])]
    Symbol(&'src str),
    /// ```ignore
    /// #<attr_id>
    /// ```
    #[regex(r"#[\p{XID_Continue}_$.]+", |lex| &lex.slice()[1..])]
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
                Err(format!("Unexpected token at position {}", span.start))
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
}
