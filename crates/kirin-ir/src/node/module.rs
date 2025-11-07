use crate::language::Language;
use super::symbol::Symbol;
use super::function::{StagedFunctionInfo, SpecializedFunctionInfo};

pub struct Module<L: Language> {
    pub name: Option<Symbol>,
    pub functions: Vec<StagedFunctionInfo<L>>,
}

pub struct SpecializedModule<L: Language> {
    pub name: Option<Symbol>,
    pub functions: Vec<SpecializedFunctionInfo<L>>,
}
