use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\f]+")]
pub enum Token<'src> {
    Error,
    #[regex(r"%[\p{XID_Continue}_$.]+", |lex| &lex.slice()[1..])]
    SSAValue(&'src str),
    #[regex(r"\^[\p{XID_Continue}_$.]+", |lex| &lex.slice()[1..])]
    Block(&'src str),
    // {identifier}
    #[regex(r"\{[\p{XID_Start}_][\p{XID_Continue}_$.]*\}", |lex| &lex.slice()[1..lex.slice().len()-1])]
    Quote(&'src str),
    #[regex(r"[\p{XID_Start}_][\p{XID_Continue}_$.]*")]
    Identifier(&'src str),
    #[regex(r"@[\p{XID_Continue}_$.]+", |lex| &lex.slice()[1..])]
    Symbol(&'src str),
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
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("<")]
    LAngle,
    #[token(">")]
    RAngle,

    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("=")]
    Equal,
    #[token("->")]
    Arrow,
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
            Token::Quote(name) => write!(f, "{{{}}}", name),
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
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LAngle => write!(f, "<"),
            Token::RAngle => write!(f, ">"),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Equal => write!(f, "="),
            Token::Arrow => write!(f, "->"),
            Token::Ellipsis => write!(f, "..."),
            Token::DoubleColon => write!(f, "::"),
            Token::Semicolon => write!(f, ";"),
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
    fn test_quote() {
        let input = "{my_identifier}";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Quote("my_identifier"))));
        assert_eq!(lexer.next(), None);
    }
}
