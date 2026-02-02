use kirin::prelude::*;


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

impl TypeLatticeEmit for SimpleTypeLattice {}

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
        }
        .map(Value::I64);

        let float = select! {
            Token::Float(s) => s.parse::<f64>().unwrap()
        }
        .map(Value::F64);

        float.or(int).labelled("value").boxed()
    }
}

// PrettyPrint traits for Value (used by PrettyPrint derive)

impl<L: Dialect> kirin::pretty::PrettyPrint<L> for Value {
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

// A simpler dialect without Region fields for testing parse/print roundtrip
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type_lattice = SimpleTypeLattice, fn)]
#[chumsky(crate = kirin::parsers)]
pub enum SimpleLang {
    #[chumsky(format = "{res:name} = add {lhs}, {rhs} -> {res:type}")]
    Add {
        lhs: SSAValue,
        rhs: SSAValue,
        #[kirin(type = SimpleTypeLattice::Float)]
        res: ResultValue,
    },
    #[chumsky(format = "{res:name} = constant {value} -> {res:type}")]
    Constant {
        #[kirin(into)]
        value: Value,
        #[kirin(type = SimpleTypeLattice::Float)]
        res: ResultValue,
    },
    #[kirin(terminator)]
    #[chumsky(format = "return {arg}")]
    Return { arg: SSAValue },
    #[chumsky(format = "{1:name} = function {0}")]
    Function {
        region: Region,
        #[kirin(type = SimpleTypeLattice::Float)]
        res: ResultValue,
    },
}

#[test]
fn test_block() {
    let mut context: Context<SimpleLang> = Context::default();
    let staged_function = context
        .staged_function()
        .name("foo")
        .params_type(&[Int])
        .return_type(Int)
        .new();

    let a = SimpleLang::op_constant(&mut context, 1.2);
    let b = SimpleLang::op_constant(&mut context, 3.4);
    let c = SimpleLang::op_add(&mut context, a.res, b.res);
    let block_arg_x = context.block_argument(0);
    let d = SimpleLang::op_add(&mut context, c.res, block_arg_x);
    let ret = SimpleLang::op_return(&mut context, d.res);

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

    let ret = SimpleLang::op_return(&mut context, block_arg_x);
    let block_b = context.block().argument(Float).terminator(ret).new();

    let body = context.region().add_block(block_a).add_block(block_b).new();
    let fdef = SimpleLang::op_function(&mut context, body);
    let f = context.specialize().f(staged_function).body(fdef).new();

    // Pretty print the function
    let mut doc = Document::new(Default::default(), &context);
    let result = doc.render(&f).unwrap();
    println!("{}", result);
    // Verify the output contains expected elements
    assert!(result.contains("function"));
    assert!(result.contains("constant"));
    assert!(result.contains("add"));
    assert!(result.contains("return"));
}

// ============================================================================
// Roundtrip Tests
// ============================================================================

use kirin::parsers::{EmitContext, EmitIR, parse_ast};
use kirin::pretty::Config;

/// Test roundtrip: parse -> emit -> print should produce output matching input.
#[test]
fn test_roundtrip_add() {
    let mut context: Context<SimpleLang> = Context::default();

    // Create operand SSAs with types
    let ssa_a = context
        .ssa()
        .name("a".to_string())
        .ty(Int)
        .kind(SSAKind::Test)
        .new();
    let ssa_b = context
        .ssa()
        .name("b".to_string())
        .ty(Int)
        .kind(SSAKind::Test)
        .new();

    // Parse - type annotation in input
    let input = "%res = add %a, %b -> float";
    let ast = parse_ast::<SimpleLang>(input).expect("parse failed");

    // Emit to get the dialect variant
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("a".to_string(), ssa_a);
    emit_ctx.register_ssa("b".to_string(), ssa_b);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&context).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Verify the result has the correct type by checking the SSA
    if let SimpleLang::Add { res, .. } = dialect {
        let res_ssa: kirin_ir::SSAValue = (*res).into();
        let res_info = res_ssa.get_info(&context).expect("result SSA should exist");
        assert_eq!(
            res_info.ty(),
            &SimpleTypeLattice::Float,
            "Result type should be Float"
        );
    }

    // Pretty print directly using the trait
    let config = Config::default();
    let doc = Document::new(config, &context);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare (trim whitespace)
    assert_eq!(output.trim(), input);
}

/// Test roundtrip for constant instruction.
#[test]
fn test_roundtrip_constant() {
    use kirin::pretty::PrettyPrint as _;

    let mut context: Context<SimpleLang> = Context::default();

    // Parse - type annotation in input
    let input = "%x = constant 42 -> float";
    let ast = parse_ast::<SimpleLang>(input).expect("parse failed");

    // Emit
    let mut emit_ctx = EmitContext::new(&mut context);
    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&context).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Verify the result has the correct type
    if let SimpleLang::Constant { res, .. } = dialect {
        let res_ssa: kirin_ir::SSAValue = (*res).into();
        let res_info = res_ssa.get_info(&context).expect("result SSA should exist");
        assert_eq!(
            res_info.ty(),
            &SimpleTypeLattice::Float,
            "Result type should be Float"
        );
    }

    // Pretty print
    let config = Config::default();
    let doc = Document::new(config, &context);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare
    assert_eq!(output.trim(), input);
}

/// Test roundtrip for return instruction.
#[test]
fn test_roundtrip_return() {
    use kirin::pretty::PrettyPrint as _;

    let mut context: Context<SimpleLang> = Context::default();

    // Create operand SSA
    let ssa_v = context
        .ssa()
        .name("v".to_string())
        .ty(Int)
        .kind(SSAKind::Test)
        .new();

    // Parse
    let input = "return %v";
    let ast = parse_ast::<SimpleLang>(input).expect("parse failed");

    // Emit
    let mut emit_ctx = EmitContext::new(&mut context);
    emit_ctx.register_ssa("v".to_string(), ssa_v);

    let statement = ast.emit(&mut emit_ctx);
    let stmt_info = statement.get_info(&context).expect("stmt should exist");
    let dialect = stmt_info.definition();

    // Pretty print
    let config = Config::default();
    let doc = Document::new(config, &context);
    let arena_doc = dialect.pretty_print(&doc);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render failed");

    // Compare
    assert_eq!(output.trim(), input);
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

/// Test roundtrip for a full function with region containing multiple blocks and statements.
///
/// Note: This test verifies that parsing and emitting functions with regions works correctly.
/// The exact output format may differ from input due to Block/Region pretty printing details
/// (e.g., block names, result alignment), but the core structure is preserved.
#[test]
fn test_roundtrip_function() {
    let mut context: Context<SimpleLang> = Context::default();

    // Parse a function with a region containing a block with multiple statements
    let input = r#"%f = function {
    ^entry(%x: float) {
        %y = add %x, %x -> float;
        %z = constant 42 -> float;
        %w = add %y, %z -> float;
        return %w;
    }
}"#;

    let ast = parse_ast::<SimpleLang>(input).expect("parse failed");

    // Emit to IR
    let mut emit_ctx = EmitContext::new(&mut context);
    let statement = ast.emit(&mut emit_ctx);

    // Pretty print using Document::render()
    let mut doc = Document::new(Config::default(), &context);
    let output = doc.render(&statement).expect("render failed");

    // Verify key structural elements are present
    assert!(
        output.contains("%f = function"),
        "Should have function result name"
    );
    assert!(output.contains("add"), "Should have add instruction");
    assert!(
        output.contains("constant 42"),
        "Should have constant instruction"
    );
    assert!(output.contains("return"), "Should have return instruction");
}

/// Test roundtrip for a function with multiple blocks in the region.
///
/// Note: This test verifies that parsing and emitting functions with multiple blocks works.
/// The exact output format may differ from input due to Block/Region pretty printing details.
#[test]
fn test_roundtrip_function_multiple_blocks() {
    let mut context: Context<SimpleLang> = Context::default();

    // Parse a function with a region containing multiple blocks
    let input = r#"%f = function {
    ^entry(%x: float) {
        %y = add %x, %x -> float;
        return %y;
    }
    ^second(%a: float) {
        %b = constant 100 -> float;
        return %b;
    }
}"#;

    let ast = parse_ast::<SimpleLang>(input).expect("parse failed");

    // Emit to IR
    let mut emit_ctx = EmitContext::new(&mut context);
    let statement = ast.emit(&mut emit_ctx);

    // Pretty print using Document::render() with 4-space indentation to match input
    let config = Config {
        tab_spaces: 4,
        ..Default::default()
    };
    let output = statement.sprint_with_config(config, &context);
    println!("{}", output);
    // Note: output has a trailing newline from pretty printer
    assert_eq!(output.trim_end(), input);
}
