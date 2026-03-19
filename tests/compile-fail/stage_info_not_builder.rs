/// Passing `&mut StageInfo` to a derive-generated builder function should
/// produce a helpful error pointing users to `with_builder()`.
use kirin::ir::{BuilderSSAKind, StageInfo};
use kirin_test_languages::SimpleLanguage;

fn main() {
    let mut stage: StageInfo<SimpleLanguage> = StageInfo::default();
    let ssa = stage.with_builder(|b| {
        b.ssa(None::<String>, kirin_test_languages::SimpleType::F64, BuilderSSAKind::Test)
    });
    SimpleLanguage::op_add(&mut stage, ssa, ssa);
}
