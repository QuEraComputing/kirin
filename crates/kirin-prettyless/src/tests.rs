use super::*;
use kirin_test_utils::*;

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
    insta::assert_snapshot!(doc.render(&f).unwrap());
}
