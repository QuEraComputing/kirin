# Create a derive macro for the parser trait

## Previous Work

We have some experimental implementations with notation `-old` suffix in the `crates` directory.

`kirin-chumsky-old` is the oldest version, it contains definitions of the traits for a dialect object (an enum whose variants are the statements, or a struct who is the only statement in the dialect). Then a set of parsers for common syntaxes are implemented. The pair crate `kirin-chumsky-derive-old` provides the derive macros for the traits that implements the parser trait by combining parsers based on the statement definition.

`kirin-chumsky-2-old` is the 2nd iteration of the `kirin-chumsky-old` crate. It implements a common set of syntaxes, their corresponding syntax trees
and the chumsky parsers for them. In `kirin-chumsky-format-old` crate, we try to implement a set of pipelines that process the derive input and generate an AST
structure that is built on top of the syntax nodes defined in `kirin-chumsky-2-old` crate. Then we generate the parser trait implementation by combining the parsers based on the AST structure.

However, we were not able to complete the implementation due to the complexity of the problem.

## New Work

In general, we want to implement the following derive macro:

```rust
#[derive(HasRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = Type)] // The type lattice this dialect uses
#[chumsky(crate = kirin_chumsky)] // The crate path to the chumsky parsers
pub enum MyDialectAST
{
    #[chumsky(format = "{res:name} = add {lhs} {rhs} -> {res:type}")] // The format string for this statement
    Add(SSAValue, SSAValue, ResultValue),
    #[chumsky(format = "{res:name} = mul {lhs:name} {lhs:type} {rhs} -> {res:type}")] // The format string for this statement
    Mul(SSAValue, SSAValue, ResultValue),
    #[chumsky(format = "return {0}")] // The format string for this statement
    Return(SSAValue),
}
```

the `WithAbstractSyntaxTree` trait is a trait that says this dialect object implements an abstract syntax tree type built on top of some pre-defined syntax nodes, for example, we want to generate an AST type for the `MyDialectAST` type that is built on top of the `SSAValue`, `ResultValue` and `Block` syntax nodes, the generated AST type looks like follows:

```rust
pub enum MyDialectAST<'tokens, 'src: 'tokens, L: Dialect + 'tokens>
where
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    Add(SSAValue<'tokens, 'src, L>, SSAValue<'tokens, 'src, L>, ResultValue<'tokens, 'src, L>),
    Mul(SSAValue<'tokens, 'src, L>, SSAValue<'tokens, 'src, L>, ResultValue<'tokens, 'src, L>),
    Return(SSAValue<'tokens, 'src, L>),
}
```

For example, the following syntax:

```llvm
%result = add %a %b -> int
```

with a format string of `{res:name} = add {lhs} {rhs} -> {res:type}`,

which is parsed as follows:

```rust
Add {
    lhs: SSAValue { name: "a", ty: None },
    rhs: SSAValue { name: "b", ty: None },
    res: ResultValue { name: "result", ty: /* what ever <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output is, e.g Some(Int) */}
}
```

here we assume `L::TypeLattice` implements the `HasParser` trait for a language `L` (which is also a dialect object). We assume the type lattice syntax is not recursive, so we can always get a parser for the type lattice syntax by calling `L::TypeLattice::parser()`. On the other hand, we restrict the tokenizer implementation to be the `kirin_lexer` crate, so the token stream is always a stream of `Token`s defined in the `kirin_lexer` crate. The token stream contains a view of the input source code and thus has a lifetime bound `'tokens` which is the same as the lifetime bound of the input source code `'src`.

where `HasRecursiveParser` is the a trait that says this dialect object implements a recursive parser that takes a parser of the entire language (which is a composition of dialects) and returns a parser of this dialect object. This is useful when the statement definition contains recursive syntaxes such as blocks or regions (e.g a function definition).

At the top level, we assume if we have an implementation of `HasRecursiveParser` for a dialect object, we automatically have an implementation of `HasParser` for the dialect object (by assuming the dialect parser recursively calls itself to parse the nested syntaxes). For example, if we have our final language defined as composition of dialects:

```rust
#[derive(HasRecursiveParser)]
#[kirin(type_lattice = Type)]
#[chumsky(crate = kirin_chumsky)]
pub enum MyLanguageAST
{
    Arith(ArithDialect),
    ControlFlow(ControlFlowDialect),
    Function(FunctionDialect),
}
```

the trait `HasParser` should be automatically implemented for the `MyLanguageAST` type (it's an auto-trait). Then one can use `MyLanguageAST::parser()` to get a parser of the entire language. Internally, it just use the `recursive` combinator to parse the nested syntaxes.

### Blocks and Regions

For blocks we assume the following syntax:

```llvm
^bb0(%arg: i32) {
    %x = add %arg, %arg;
    return %x;
}
```

so the syntax of a block can not be customized, it always has a label, a list of arguments and a list of statements. The arguments always expect a type annotation.

```rust
BlockAST {
    header: BlockHeader,
    statements: Vec<StatementAST>,
}
```

there should be a combinator to parse a block, and if we generate the parser, we will always assume using this syntax for blocks (e.g the statement field is of type `kirin_ir::Block`). User can customize the syntax by providing a custom `HasRecursiveParser` implementation for the dialect object.

For regions we assume the following syntax:

```llvm
{
    ^bb0(%arg: i32) {
        %x = add %arg, %arg;
        return %x;
    }
}
```

so the syntax of a region can not be customized, it always has a list of blocks.

```rust
RegionAST {
    blocks: Vec<BlockAST>,
}
```

there should be a combinator to parse a region, and if we generate the parser, we will always assume using this syntax for regions (e.g the statement field is of type `kirin_ir::Region`). User can customize the syntax by providing a custom `HasRecursiveParser` implementation for the dialect object.

## Steps

1. First, we create a new crate `kirin-chumsky`, which contains the runtime API for the chumsky parsers, e.g the trait `HasRecursiveParser`, `HasParser`, and `WithAbstractSyntaxTree`, a common set of syntax nodes that are shared by all dialects, and a set of combinators for composing parsers.
2. Then we create a new crate `kirin-chumsky-format`, which contains the logic for parsing the format string and generate the AST type for the dialect object, as well as the derive macro for the `HasRecursiveParser` and `WithAbstractSyntaxTree` traits.
3. Then we create a new crate `kirin-chumsky-derive`, which contains the derive macro for the `HasRecursiveParser` and `WithAbstractSyntaxTree` traits (this is only a simple wrapper around the `kirin-chumsky-format` crate to allow people reuse the derive macro implementation elsewhere).
4. Write some unit tests for the `kirin-chumsky` crate, e.g the combinators for parsing identifiers, symbols, blocks, regions, etc.
5. Finally, we create integration tests for the derive macro in the `kirin-chumsky-derive` crate.
6. Run `cargo fmt` and `cargo build` to make sure the code is formatted and builds, fix any issues.
7. Run all the tests and make sure they pass.

## Tools

We use `cargo` to build the crates and run the tests, we use `insta` to test the generated code matches the expected code in unit tests. The integration tests should be actually using the derive macros and test the runtime API with strings and parsed ASTs.
