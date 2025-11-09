use crate::language::Language;
use super::symbol::Symbol;
use super::function::{StagedFunctionInfo, SpecializedFunctionInfo};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Module<L: Language> {
    pub name: Option<Symbol>,
    pub functions: Vec<StagedFunctionInfo<L>>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecializedModule<L: Language> {
    pub name: Option<Symbol>,
    pub functions: Vec<SpecializedFunctionInfo<L>>,
}
