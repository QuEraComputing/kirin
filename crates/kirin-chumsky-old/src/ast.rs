/// Some common AST structures for downstream dialect
/// to use with chumsky parsers.
use kirin_ir::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: chumsky::span::SimpleSpan,
}

impl<T: Copy> Copy for Spanned<T> {}

impl<T: std::fmt::Display> std::fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct FunctionType<T> {
    pub input_types: Vec<Spanned<T>>,
    pub output_types: Vec<Spanned<T>>,
}

impl<T: PartialEq> PartialEq for FunctionType<T> {
    fn eq(&self, other: &Self) -> bool {
        self.input_types == other.input_types && self.output_types == other.output_types
    }
}

#[derive(Debug, Clone)]
pub struct Operand<'src, T> {
    pub name: Spanned<&'src str>,
    /// the type of the result value, if specified
    pub ty: Option<T>,
}

impl<'src, T: PartialEq> PartialEq for Operand<'src, T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.ty == other.ty
    }
}

#[derive(Debug, Clone)]
pub struct ResultValue<'src, T> {
    pub name: Spanned<&'src str>,
    /// the type of the result value, if specified
    pub ty: Option<T>,
}

impl<'src, T: PartialEq> PartialEq for ResultValue<'src, T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.ty == other.ty
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockLabel<'src> {
    pub name: Spanned<&'src str>,
}

#[derive(Debug, Clone)]
pub struct BlockArgument<'src, T> {
    pub name: Spanned<&'src str>,
    pub ty: Spanned<T>,
}

impl<'src, T: PartialEq> PartialEq for BlockArgument<'src, T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.ty == other.ty
    }
}

#[derive(Debug, Clone)]
pub struct BlockHeader<'src, T> {
    pub label: BlockLabel<'src>,
    pub arguments: Vec<Spanned<BlockArgument<'src, T>>>,
}

impl<'src, T: PartialEq> PartialEq for BlockHeader<'src, T> {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && self.arguments == other.arguments
    }
}

#[derive(Debug, Clone)]
pub struct Block<'src, T, S> {
    pub header: Spanned<BlockHeader<'src, T>>,
    pub statements: Vec<Spanned<S>>,
}

impl<'src, T: PartialEq, S: PartialEq> PartialEq for Block<'src, T, S> {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header && self.statements == other.statements
    }
}

#[derive(Debug, Clone)]
pub struct Region<'src, T, S> {
    pub blocks: Vec<Spanned<Block<'src, T, S>>>,
}

impl<'src, T: PartialEq, S: PartialEq> PartialEq for Region<'src, T, S> {
    fn eq(&self, other: &Self) -> bool {
        self.blocks == other.blocks
    }
}
