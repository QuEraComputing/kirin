# kirin-chumsky-derive

Procedural macros for deriving parser traits for Kirin dialect IR.

## Overview

This crate provides derive macros to automatically generate implementations of:
- `HasParser` - Generates chumsky parsers for dialect AST types
- `WithAbstractSyntaxTree` - Maps IR types to their AST representations

These macros enable you to define the text format of your dialect using a simple format string DSL, and the implementation will automatically compose parsers for dialect composition.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
kirin-chumsky-derive = "0.1"
kirin-chumsky = "0.1"
kirin-ir = "0.1"
```

## `#[derive(HasParser)]`

Generates a parser implementation that can parse text into your AST type.

### Basic Example

```rust
use kirin_chumsky::{prelude::*, ast};
use kirin_chumsky_derive::HasParser;
use kirin_ir::*;

#[derive(Clone, Debug, PartialEq, HasParser)]
#[kirin(type_lattice = MyTypeLattice, crate = kirin_ir)]
#[chumsky(crate = kirin_chumsky)]
pub enum MyDialectAST<'tokens, 'src: 'tokens, L: Dialect + 'tokens>
where
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    #[chumsky(format = "{res} = add {lhs}, {rhs}")]
    Add {
        lhs: ast::Operand<'tokens, 'src, L>,
        rhs: ast::Operand<'tokens, 'src, L>,
        res: ast::ResultValue<'tokens, 'src, L>,
    },

    #[chumsky(format = "return {0}")]
    Return(ast::Operand<'tokens, 'src, L>),
}
```

This generates a parser that can parse:
```
%result = add %lhs, %rhs
return %value
```

### Format String DSL

Format strings define the textual syntax of each statement variant:

- **Literal tokens**: `add`, `=`, `,` - matched exactly
- **Field placeholders**: `{field_name}` - parsed using the field's `HasParser` implementation
- **Positional fields**: `{0}`, `{1}` - for tuple struct/variant fields

#### Supported Field Types

The derive macro recognizes these IR field types and automatically generates appropriate parsers:

| IR Type | AST Type | Parser Used |
|---------|----------|-------------|
| `SSAValue` | `ast::Operand` | `operand()` |
| `ResultValue` | `ast::ResultValue` | `result_value()` |
| `Block` | `ast::Block` | `block()` |
| `Successor` | `ast::BlockLabel` | `block_label()` |
| `Region` | `ast::Region` | `region()` |
| Custom types | Self | `<T as HasParser<L>>::parser()` |

#### Field Collections

Collections are automatically supported:
- `Vec<T>` - Parses multiple values
- `Option<T>` - Parses optional values

### Attributes

#### Global Attributes

On the type definition:

```rust
#[chumsky(crate = path::to::kirin_chumsky)]  // Override crate path
#[chumsky(format = "default format")]         // Default format for all variants
```

#### Statement/Variant Attributes

On each variant or struct:

```rust
#[chumsky(format = "{res} = op {arg}")]      // Format for this variant
```

### Generated Code

For the `Add` variant above, the macro generates:

```rust
impl<'tokens, 'src: 'tokens, L> HasParser<'tokens, 'src, L> for MyDialectAST<'tokens, 'src, L>
where
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type Output = Self;
    fn parser<I: TokenInput<'tokens, 'src>>()
        -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>>
    {
        choice((
            // Add variant parser
            <ast::Operand as HasParser<L>>::parser()  // lhs
                .then_ignore(just(Token::Equal))
                .then_ignore(just(Token::Identifier("add")))
                .then(<ast::Operand as HasParser<L>>::parser())  // rhs
                .then_ignore(just(Token::Comma))
                .then(<ast::ResultValue as HasParser<L>>::parser())  // res
                .map(|((lhs, rhs), res)| MyDialectAST::Add { lhs, rhs, res }),

            // Return variant parser
            // ... similar structure
        )).boxed()
    }
}
```

## `#[derive(WithAbstractSyntaxTree)]`

Maps IR types to their corresponding AST node types. For dialect AST types, this typically maps to `Self`.

### Example

```rust
use kirin_chumsky_derive::{HasParser, WithAbstractSyntaxTree};

#[derive(Clone, Debug, PartialEq, HasParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = MyTypeLattice)]
#[chumsky(crate = kirin_chumsky)]
pub enum MyDialectAST<'tokens, 'src: 'tokens, L: Dialect + 'tokens>
where
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    // ... variants
}
```

This generates:

```rust
impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for MyDialectAST<'tokens, 'src, L>
where
    L: Dialect,
{
    type AbstractSyntaxTreeNode = Self;
}
```

### Purpose

The `WithAbstractSyntaxTree` trait establishes the mapping between runtime IR types and their parse-time AST representations:

- Runtime types like `SSAValue`, `ResultValue`, `Block` map to AST types like `ast::Operand`, `ast::ResultValue`, `ast::Block`
- AST types map to themselves
- Custom types can override the mapping

This trait is essential for the type system to understand how parsers should be composed when dialects are nested or composed.

## Dialect Composition

The derive macros support dialect composition through the `#[wraps]` attribute (from `kirin-derive-dialect`):

```rust
use kirin_derive::Dialect;
use kirin_chumsky_derive::{HasParser, WithAbstractSyntaxTree};

#[derive(Dialect, HasParser, WithAbstractSyntaxTree)]
#[wraps]
#[kirin(type_lattice = T)]
pub enum ComposedDialect<T: TypeLattice> {
    Arith(ArithDialect),
    ControlFlow(ControlFlowDialect),
}
```

When a dialect wraps other dialects, the parsers automatically compose - the outer dialect's parser becomes a `choice()` over the inner dialects' parsers.

## Architecture

The implementation is split across three crates:

1. **kirin-chumsky-derive** (this crate): Proc macro exports
2. **kirin-chumsky-format**: Core implementation logic
   - Format string parsing
   - Parser generation code
   - Token stream manipulation
3. **kirin-chumsky**: Runtime API
   - `HasParser` and `WithAbstractSyntaxTree` trait definitions
   - AST type definitions (`ast::Operand`, etc.)
   - Parser combinators (`operand()`, `block()`, etc.)

This separation allows:
- Clean proc macro interface
- Testable core logic without proc macro machinery
- Reusable runtime components

## Complete Example

```rust
use kirin_chumsky::{prelude::*, ast};
use kirin_chumsky_derive::{HasParser, WithAbstractSyntaxTree};
use kirin_ir::*;
use kirin_lexer::{Token, Logos};
use chumsky::input::Stream;

// Define your type lattice
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MyLattice {
    Int, Float, Unit
}

impl Lattice for MyLattice {
    fn is_subseteq(&self, other: &Self) -> bool { self == other }
    fn join(&self, _other: &Self) -> Self { self.clone() }
    fn meet(&self, _other: &Self) -> Self { self.clone() }
}

impl FiniteLattice for MyLattice {
    fn bottom() -> Self { MyLattice::Unit }
    fn top() -> Self { MyLattice::Int }
}

impl TypeLattice for MyLattice {}

// Make it parseable
impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src, MyDialect> for MyLattice {
    type Output = MyLattice;
    fn parser<I: TokenInput<'tokens, 'src>>()
        -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>>
    {
        // Implement type parsing logic
        empty().map(|_| MyLattice::Unit).boxed()
    }
}

// Define your dialect AST
#[derive(Clone, Debug, PartialEq, HasParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = MyLattice)]
#[chumsky(crate = kirin_chumsky)]
pub enum MyDialectAST<'tokens, 'src: 'tokens, L: Dialect + 'tokens>
where
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    #[chumsky(format = "{res} = add {lhs}, {rhs}")]
    Add {
        lhs: ast::Operand<'tokens, 'src, L>,
        rhs: ast::Operand<'tokens, 'src, L>,
        res: ast::ResultValue<'tokens, 'src, L>,
    },

    #[chumsky(format = "{res} = mul {lhs}, {rhs}")]
    Mul {
        lhs: ast::Operand<'tokens, 'src, L>,
        rhs: ast::Operand<'tokens, 'src, L>,
        res: ast::ResultValue<'tokens, 'src, L>,
    },

    #[chumsky(format = "return {0}")]
    Return(ast::Operand<'tokens, 'src, L>),
}

// Use the parser
fn main() {
    let src = "%r = add %a, %b";
    let token_iter = Token::lexer(src)
        .spanned()
        .map(|(tok, span)| match tok {
            Ok(tok) => (tok, span.into()),
            Err(()) => (Token::Error, span.into()),
        });

    let len = src.len();
    let token_stream = Stream::from_iter(token_iter)
        .map((0..len).into(), |(t, s)| (t, s));

    let parser = MyDialectAST::<MyDialect>::parser();
    let result = parser.parse(token_stream).into_result();

    match result {
        Ok(ast) => println!("Parsed: {:?}", ast),
        Err(errs) => {
            for e in errs {
                eprintln!("Parse error: {:?}", e);
            }
        }
    }
}
```

## Limitations

1. **Format DSL is minimal**: Only supports basic token matching and field interpolation. For complex parsing (e.g., operator precedence, custom syntax), implement `HasParser` manually.

2. **Field order matters**: Fields in the format string must match the order they appear in the source text. The parser is LL(1)-style sequential.

3. **No lookahead/backtracking control**: The generated parser uses chumsky's default behavior. For fine-grained control, implement `HasParser` manually.

4. **Built-in types have manual implementations**: Core types like `SSAValue` and `ResultValue` have hand-written `HasParser` implementations to avoid circular dependencies and provide stable semantics.

## See Also

- **kirin-derive**: Derives `Dialect` trait with field iterators, properties, and builder pattern
- **kirin-derive-core-2**: Shared IR infrastructure for derive macros
- **kirin-chumsky**: Runtime parser library with trait definitions and combinators
