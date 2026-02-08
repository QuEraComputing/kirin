use kirin_ir::{Dialect, SSAKind, SSAValue, StageInfo};

pub fn new_test_ssa<L: Dialect>(
    stage: &mut StageInfo<L>,
    name: impl Into<String>,
    ty: L::Type,
) -> SSAValue {
    stage
        .ssa()
        .name(name.into())
        .ty(ty)
        .kind(SSAKind::Test)
        .new()
}
