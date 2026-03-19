use kirin_ir::{BuilderSSAKind, BuilderStageInfo, Dialect, SSAValue};

pub fn new_test_ssa<L: Dialect>(
    stage: &mut BuilderStageInfo<L>,
    name: impl Into<String>,
    ty: L::Type,
) -> SSAValue {
    stage.ssa(Some(name.into()), ty, BuilderSSAKind::Test)
}
