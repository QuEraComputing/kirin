use kirin_ir::{BuilderSSAKind, BuilderStageInfo, Dialect, SSAValue};

pub fn new_test_ssa<L: Dialect>(
    stage: &mut BuilderStageInfo<L>,
    name: impl Into<String>,
    ty: L::Type,
) -> SSAValue {
    stage
        .ssa()
        .name(name.into())
        .ty(ty)
        .kind(BuilderSSAKind::Test)
        .new()
}
