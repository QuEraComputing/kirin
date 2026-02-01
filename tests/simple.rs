use kirin::ir::*;
use kirin::parsers::{BoxedParser, HasParser, TokenInput};
use kirin::parsers::chumsky::prelude::*;
use kirin::parsers::Token;
use kirin::pretty::{Document, ArenaDoc, DocAllocator, PrettyPrint, PrettyPrintName, PrettyPrintType};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum SimpleTypeLattice {
    Any,
    Int,
    Float,
    DataType,
    Bottom,
}

pub use SimpleTypeLattice::*;

impl Lattice for SimpleTypeLattice {
    fn is_subseteq(&self, other: &Self) -> bool {
        matches!((self, other), (a, b) if a == b)
    }

    fn join(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            other.clone()
        } else if other.is_subseteq(self) {
            self.clone()
        } else {
            SimpleTypeLattice::Any
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            self.clone()
        } else if other.is_subseteq(self) {
            other.clone()
        } else {
            SimpleTypeLattice::Bottom
        }
    }
}

impl FiniteLattice for SimpleTypeLattice {
    fn bottom() -> Self {
        SimpleTypeLattice::Bottom
    }

    fn top() -> Self {
        SimpleTypeLattice::Any
    }
}

impl crate::TypeLattice for SimpleTypeLattice {}

impl std::fmt::Display for SimpleTypeLattice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleTypeLattice::Any => write!(f, "any"),
            SimpleTypeLattice::Int => write!(f, "int"),
            SimpleTypeLattice::Float => write!(f, "float"),
            SimpleTypeLattice::DataType => write!(f, "datatype"),
            SimpleTypeLattice::Bottom => write!(f, "bottom"),
        }
    }
}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for SimpleTypeLattice {
    type Output = SimpleTypeLattice;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        select! {
            Token::Identifier("any") => SimpleTypeLattice::Any,
            Token::Identifier("int") => SimpleTypeLattice::Int,
            Token::Identifier("float") => SimpleTypeLattice::Float,
            Token::Identifier("datatype") => SimpleTypeLattice::DataType,
            Token::Identifier("bottom") => SimpleTypeLattice::Bottom,
        }
        .labelled("type")
        .boxed()
    }
}

impl Typeof<SimpleTypeLattice> for i64 {
    fn type_of(&self) -> SimpleTypeLattice {
        SimpleTypeLattice::Int
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I64(i64),
    F64(f64),
}
impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::I64(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            Value::F64(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
        }
    }
}

impl Typeof<SimpleTypeLattice> for Value {
    fn type_of(&self) -> SimpleTypeLattice {
        match self {
            Value::I64(_) => SimpleTypeLattice::Int,
            Value::F64(_) => SimpleTypeLattice::Float,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::I64(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::I64(v) => write!(f, "{}", v),
            Value::F64(v) => write!(f, "{}", v),
        }
    }
}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for Value {
    type Output = Value;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        let int = select! {
            Token::Int(s) => s.parse::<i64>().unwrap()
        }.map(Value::I64);

        let float = select! {
            Token::Float(s) => s.parse::<f64>().unwrap()
        }.map(Value::F64);

        float.or(int).labelled("value").boxed()
    }
}

// PrettyPrint traits for Value (used by PrettyPrint derive)

impl<L: Dialect> PrettyPrint<L> for Value {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        doc.text(self.to_string())
    }
}

impl<L: Dialect> PrettyPrintName<L> for Value {
    fn pretty_print_name<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        doc.text(self.to_string())
    }
}

impl<L: Dialect> PrettyPrintType<L> for Value {
    fn pretty_print_type<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        // Value doesn't have a separate type - use empty or the type of the value
        match self {
            Value::I64(_) => doc.text("int"),
            Value::F64(_) => doc.text("float"),
        }
    }
}

// Note: Region fields require manual PrettyPrint implementation since the derive
// doesn't yet support the required EmitIR bounds for Region's nested statements.
#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(fn, type_lattice = SimpleTypeLattice)]
pub enum SimpleLanguage {
    Add(
        SSAValue,
        SSAValue,
        #[kirin(type = SimpleTypeLattice::Float)] ResultValue,
    ),
    Constant(
        #[kirin(into)] Value,
        #[kirin(type = SimpleTypeLattice::Float)] ResultValue,
    ),
    #[kirin(terminator)]
    Return(SSAValue),
    Function(
        Region,
        #[kirin(type = SimpleTypeLattice::Float)] ResultValue,
    ),
}

// Manual PrettyPrint implementation for SimpleLanguage since we have Region fields
impl PrettyPrint<SimpleLanguage> for SimpleLanguage {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, SimpleLanguage>) -> ArenaDoc<'a> {
        match self {
            SimpleLanguage::Add(lhs, rhs, _) => {
                doc.text(format!("add {}, {}", *lhs, *rhs))
            }
            SimpleLanguage::Constant(value, _) => {
                doc.text(format!("constant {}", value))
            }
            SimpleLanguage::Return(retval) => {
                doc.text(format!("return {}", *retval))
            }
            SimpleLanguage::Function(region, _) => {
                let region_doc = region.pretty_print(doc);
                doc.text("function ").append(region_doc)
            }
        }
    }
}

// Note: Parsing tests for SimpleLanguage are skipped because the HasParser derive
// doesn't support Region fields yet. See kirin-chumsky-derive/tests for parsing tests
// with dialects that don't use Region fields.

#[test]
fn test_block() {
    let mut context: Context<SimpleLanguage> = Context::default();
    let staged_function = context
        .staged_function()
        .name("foo")
        .params_type(&[Int])
        .return_type(Int)
        .new();

    let a = SimpleLanguage::op_constant(&mut context, 1.2);
    let b = SimpleLanguage::op_constant(&mut context, 3.4);
    let c = SimpleLanguage::op_add(&mut context, a.result, b.result);
    let block_arg_x = context.block_argument(0);
    let d = SimpleLanguage::op_add(&mut context, c.result, block_arg_x);
    let ret = SimpleLanguage::op_return(&mut context, d.result);

    let block_a: Block = context
        .block()
        .argument(Int)
        .argument_with_name("y", Float)
        .stmt(a)
        .stmt(b)
        .stmt(c)
        .stmt(d)
        .terminator(ret)
        .new();

    let ret = SimpleLanguage::op_return(&mut context, block_arg_x);
    let block_b = context.block().argument(Float).terminator(ret).new();

    let body = context.region().add_block(block_a).add_block(block_b).new();
    let fdef = SimpleLanguage::op_function(&mut context, body);
    let f = context.specialize().f(staged_function).body(fdef).new();

    // Pretty print the function
    let mut doc = Document::new(Default::default(), &context);
    let result = doc.render(f).unwrap();
    
    // Verify the output contains expected elements
    assert!(result.contains("function"));
    assert!(result.contains("constant"));
    assert!(result.contains("add"));
    assert!(result.contains("return"));
}

/// Strip trailing whitespace in each line of the input string.
pub fn strip_trailing_whitespace(s: &str) -> String {
    if s.is_empty() {
        return "\n".to_string();
    }
    let mut res = String::with_capacity(s.len());
    for line in s.lines() {
        res.push_str(line.trim_end());
        res.push('\n');
    }
    res
}
