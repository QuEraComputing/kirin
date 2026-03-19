/// Passing `&mut StageInfo` to a derive-generated builder function should
/// produce a helpful error pointing users to `with_builder()`.
use kirin::ir::{BuilderSSAKind, StageInfo};
use kirin_test_languages::SimpleLanguage;

fn main() {
    let mut stage: StageInfo<SimpleLanguage> = StageInfo::default();
    let ssa = stage.with_builder(|b| {
        b.ssa()
            .ty(kirin_test_languages::SimpleType::F64)
            .kind(BuilderSSAKind::Test)
            .new()
    });
    SimpleLanguage::op_add(&mut stage, ssa, ssa);
}
