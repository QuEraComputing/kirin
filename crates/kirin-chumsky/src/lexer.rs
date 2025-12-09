use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\f]+")]
pub enum Token<'src> {
    Error,
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse().ok())]
    Integer(i64),
    #[regex(r"-?[0-9]+\.[0-9]+", |lex| lex.slice().parse().ok())]
    Float(f64),
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier(&'src str),
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("->")]
    Arrow,
    #[token(";")]
    Semicolon,
    #[token("=")]
    Equal,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Asterisk,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("^")]
    Caret,
    #[token("@")]
    At,
    #[token("&")]
    Ampersand,
    #[token("|")]
    Pipe,
    #[token("!")]
    Exclamation,
    #[token("<")]
    LessThan,
    #[token(">")]
    GreaterThan,
    #[token("<=")]
    LessEqual,
    #[token(">=")]
    GreaterEqual,
    #[token("==")]
    EqualEqual,
    #[token("!=")]
    NotEqual,
    #[token("fn")]
    Fn,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("repeat")]
    Repeat,
    #[token("in")]
    In,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("yield")]
    Yield,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(".")]
    Dot,
    #[token("::")]
    DoubleColon,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lexer() {
        let input = "%x, %y: i32 -> i32";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("y"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), None);

        let input = "%x = addi %y, 42";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("x"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Equal)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("addi"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("y"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Integer(42))));
        assert_eq!(lexer.next(), None);

        let input = "fn example(%arg0: i32) -> i32 { return %arg0 + 1; }";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Fn)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("example"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("arg0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Arrow)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LBrace)));
        assert_eq!(lexer.next(), Some(Ok(Token::Return)));
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("arg0"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Plus)));
        assert_eq!(lexer.next(), Some(Ok(Token::Integer(1))));
        assert_eq!(lexer.next(), Some(Ok(Token::Semicolon)));
        assert_eq!(lexer.next(), Some(Ok(Token::RBrace)));
        assert_eq!(lexer.next(), None);

        let input = "^block(%arg1: i32, %arg2: f64)";
        let mut lexer = Token::lexer(input);
        assert_eq!(lexer.next(), Some(Ok(Token::Caret)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("block"))));
        assert_eq!(lexer.next(), Some(Ok(Token::LParen)));
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("arg1"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("i32"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Comma)));
        assert_eq!(lexer.next(), Some(Ok(Token::Percent)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("arg2"))));
        assert_eq!(lexer.next(), Some(Ok(Token::Colon)));
        assert_eq!(lexer.next(), Some(Ok(Token::Identifier("f64"))));
        assert_eq!(lexer.next(), Some(Ok(Token::RParen)));
        assert_eq!(lexer.next(), None);
    }
}
