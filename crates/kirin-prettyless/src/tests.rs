use super::*;
use kirin_test_utils::*;

impl PrettyPrint for SimpleLanguage {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
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
                // Now that we have L: PrettyPrint bound, we can print the region content
                doc.text("function ") + doc.print_region(region)
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
    
    // Use the new Document method API for printing IR nodes
    let doc = Document::new(Default::default(), &context);
    let arena_doc = doc.print_specialized_function(&f);
    let max_width = doc.config().max_width;
    let mut buf = String::new();
    arena_doc.render_fmt(max_width, &mut buf).unwrap();
    insta::assert_snapshot!(buf);
}
