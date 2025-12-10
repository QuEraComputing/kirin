use kirin::ir::*;
use kirin::pretty::*;

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

impl PrettyPrint<SimpleLanguage> for SimpleLanguage {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, SimpleLanguage>) -> ArenaDoc<'a> {
        match self {
            SimpleLanguage::Add(lhs, rhs, _) => {
                let doc = doc.text(format!("add {}, {}", *lhs, *rhs));
                doc
            }
            SimpleLanguage::Constant(value, _) => {
                let doc = match value {
                    Value::I64(v) => doc.text(format!("constant {}", v)),
                    Value::F64(v) => doc.text(format!("constant {}", v)),
                };
                doc
            }
            SimpleLanguage::Return(retval) => {
                let doc = doc.text(format!("return {}", *retval));
                doc
            }
            SimpleLanguage::Function(region, _) => {
                let region_doc = region.pretty_print(doc);
                let doc = doc.text("function ").append(region_doc);
                doc
            }
        }
    }
}

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
    let mut doc = Document::new(Default::default(), &context);
    doc.pager(f).unwrap();
    // println!("{}", doc.render(f).unwrap());
    // let max_width = doc.config().max_width;
    // let doc_ = doc.build(f);
    // let mut buf = String::new();
    // doc_.render_fmt(max_width, &mut buf)
    //     .unwrap();
    // let result = strip_trailing_whitespace(&buf);
    // println!("{}", result);
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
