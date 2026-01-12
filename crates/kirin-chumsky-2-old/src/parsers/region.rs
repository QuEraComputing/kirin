use chumsky::{IterParser, Parser};

use super::block::{Block, block};
use super::traits::*;

/// a region containing a sequence of blocks
#[derive(Debug, Clone, PartialEq)]
pub struct Region<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>> {
    pub blocks: Vec<Block<'tokens, 'src, Language>>,
}

pub fn region<'tokens, 'src: 'tokens, I, Language>(
    language: RecursiveParser<'tokens, 'src, I, Language::Output>,
) -> impl ChumskyParser<'tokens, 'src, I, Region<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src>,
{
    block(language)
        .repeated()
        .collect()
        .map(|blocks| Region { blocks })
        .labelled("region")
}
