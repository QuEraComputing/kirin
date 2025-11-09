use std::collections::HashMap;

use crate::language::Language;

use super::symbol::Symbol;
use super::region::Region;

/// A unique identifier for a function.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Function(usize);

/// A unique identifier for a staged function.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct StagedFunction(usize);

/// A specialized version of a function, identified by its function ID and specialization ID.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct SpecializedFunction(usize, usize);

/// Information about a function across different compilation stages.
#[derive(Clone, Debug)]
pub struct FunctionInfo<Stage> {
    id: Function,
    /// compiled versions of the function at different stages
    /// this vector should matches the the stages in context, i.e.,
    /// `context.stages[i]` provides the context for `staged_functions[i]`.
    staged_functions: HashMap<Stage, StagedFunction>,
}

#[derive(Clone, Debug)]
pub struct StagedFunctionInfo<L: Language> {
    id: StagedFunction,
    name: Option<Symbol>,
    specializations: Vec<SpecializedFunctionInfo<L>>,
}

#[derive(Clone, Debug)]
pub struct SpecializedFunctionInfo<L: Language> {
    id: SpecializedFunction,
    signature: Signature<L>,
    return_type: L::Type,
    body: Region,
    /// Functions that call this function (used for inter-procedural analyses).
    backedges: Vec<SpecializedFunction>,
}

#[derive(Clone, Debug)]
pub struct Signature<L: Language>(pub Vec<L::Type>);

impl<Stage> From<FunctionInfo<Stage>> for Function {
    fn from(fi: FunctionInfo<Stage>) -> Self {
        fi.id
    }
}

impl<L: Language> From<StagedFunctionInfo<L>> for StagedFunction {
    fn from(sfi: StagedFunctionInfo<L>) -> Self {
        sfi.id
    }
}

impl<L: Language> From<SpecializedFunctionInfo<L>> for SpecializedFunction {
    fn from(sfi: SpecializedFunctionInfo<L>) -> Self {
        sfi.id
    }
}
