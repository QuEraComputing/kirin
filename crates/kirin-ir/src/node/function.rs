use std::collections::HashMap;

use crate::Lattice;
use crate::language::Language;

use super::region::Region;
use super::symbol::Symbol;

/// A unique identifier for a compilation stage.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct CompileStage(usize);

impl CompileStage {
    pub fn id(&self) -> usize {
        self.0
    }
}

/// A unique identifier for a function.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Function(usize);

impl Function {
    pub fn id(&self) -> usize {
        self.0
    }
}

/// A unique identifier for a staged function.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct StagedFunction(usize);

impl StagedFunction {
    pub fn id(&self) -> usize {
        self.0
    }
}

/// A specialized version of a function, identified by its function ID and specialization ID.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct SpecializedFunction(usize, usize);

impl SpecializedFunction {
    pub fn id(&self) -> (StagedFunction, usize) {
        (StagedFunction(self.0), self.1)
    }
}

/// Information about a function across different compilation stages.
#[derive(Clone, Debug)]
pub struct FunctionInfo {
    id: Function,
    /// compiled versions of the function at different stages.
    ///
    /// note that compile stages may not be sequential,
    /// i.e., some stages may be skipped when a user directly programs a low-level stage
    /// language and modifies the compilation stage accordingly.
    ///
    /// Some early stages may be discarded later in the compilation pipeline to save memory.
    ///
    /// The execution will always look for the matching stage of the target execution environment
    /// e.g an interpreter will look for the staged function at the interpreter stage.
    /// but LLVM backend will look for the staged function at the LLVM IR generation stage.
    staged_functions: HashMap<CompileStage, StagedFunction>,
}

impl FunctionInfo {
    pub fn staged_functions(&self) -> &HashMap<CompileStage, StagedFunction> {
        &self.staged_functions
    }

    pub fn add_staged_function(&mut self, stage: CompileStage, func: StagedFunction) {
        self.staged_functions.insert(stage, func);
    }
}

#[derive(Clone, Debug)]
pub struct StagedFunctionInfo<L: Language> {
    id: StagedFunction,
    name: Option<Symbol>,
    signature: Signature<L>,
    return_type: L::Type,
    specializations: Vec<SpecializedFunctionInfo<L>>,
    /// Functions that call this staged function (used for inter-procedural analyses).
    /// note that the call statement must refer to the `StagedFunction` ID,
    /// if it refers to a `SpecializedFunction`, it should be recorded in the
    /// `backedges` field of `SpecializedFunctionInfo`.
    /// thus the `backedges` field of `SpecializedFunctionInfo` is always a superset of this field.
    backedges: Vec<StagedFunction>,
}

impl<L: Language> StagedFunctionInfo<L> {
    pub fn name(&self) -> Option<&Symbol> {
        self.name.as_ref()
    }

    pub fn signature(&self) -> &Signature<L> {
        &self.signature
    }

    pub fn return_type(&self) -> &L::Type {
        &self.return_type
    }

    pub fn backedges(&self) -> &Vec<StagedFunction> {
        &self.backedges
    }

    /// Get the specializations of this staged function.
    /// The specialized function signature are strictly subset of the staged function signature
    pub fn specializations(&self) -> &Vec<SpecializedFunctionInfo<L>> {
        &self.specializations
    }

    pub fn specializations_mut(&mut self) -> &mut Vec<SpecializedFunctionInfo<L>> {
        &mut self.specializations
    }

    pub fn add_specialization(&mut self, spec: SpecializedFunctionInfo<L>) {
        self.specializations.push(spec);
    }

    pub fn all_matching(&self, signature: &Signature<L>) -> Vec<&SpecializedFunctionInfo<L>> {
        let specialized = self
            .specializations
            .iter()
            .filter(|spec| {
                spec.signature().partial_cmp(signature) == Some(std::cmp::Ordering::Less)
            })
            .collect::<Vec<_>>();
        // reduce the specialized functions to the most specific ones
        specialized
            .clone()
            .into_iter()
            .filter(|spec| {
                let sig = spec.signature();
                !specialized.iter().any(|other| {
                    other.signature().partial_cmp(&sig) == Some(std::cmp::Ordering::Less)
                })
            })
            .collect::<Vec<_>>()
    }
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

impl<L: Language> PartialEq for Signature<L> {
    fn eq(&self, other: &Self) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}

impl<L: Language> PartialOrd for Signature<L> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.0.len() != other.0.len() {
            return None;
        }
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            if !a.is_subseteq(b) {
                return None;
            }
        }
        Some(std::cmp::Ordering::Less)
    }
}

impl From<FunctionInfo> for Function {
    fn from(fi: FunctionInfo) -> Self {
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

impl<L: Language> SpecializedFunctionInfo<L> {
    pub fn body(&self) -> &Region {
        &self.body
    }

    pub fn body_mut(&mut self) -> &mut Region {
        &mut self.body
    }

    pub fn return_type(&self) -> &L::Type {
        &self.return_type
    }

    pub fn signature(&self) -> &Signature<L> {
        &self.signature
    }

    pub fn backedges(&self) -> &Vec<SpecializedFunction> {
        &self.backedges
    }
}
