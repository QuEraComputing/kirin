use crate::{
    ir::{SpecializedFunctionInfo, StagedFunctionInfo, Symbol},
    language::Language,
};

pub struct Module<L: Language> {
    pub name: Option<Symbol>,
    pub functions: Vec<StagedFunctionInfo<L>>,
}

pub struct SpecializedModule<L: Language> {
    pub name: Option<Symbol>,
    pub functions: Vec<SpecializedFunctionInfo<L>>,
}
