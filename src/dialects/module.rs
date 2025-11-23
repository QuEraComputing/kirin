use crate::ir::*;

#[derive(Clone, Debug, Statement)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Module {
    pub name: Option<Symbol>,
    pub functions: Vec<StagedFunction>,
}

#[derive(Clone, Debug, Statement)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecializedModule {
    pub name: Option<Symbol>,
    pub functions: Vec<SpecializedFunction>,
}
