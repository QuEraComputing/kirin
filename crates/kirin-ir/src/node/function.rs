use std::collections::HashMap;

use crate::arena::{GetInfo, Id, Item};
use crate::language::Dialect;
use crate::{Lattice, Statement, identifier};

use super::symbol::Symbol;

identifier! {
    /// A unique identifier for a compilation stage.
    ///
    /// Compilation stages represent different phases in the compilation pipeline,
    /// such as parsing, optimization, code generation, etc.
    struct CompileStage
}

impl CompileStage {
    pub fn new(stage: Id) -> Self {
        CompileStage(stage)
    }
}

identifier! {
    /// A unique identifier for a generic function.
    ///
    /// Functions can have multiple staged versions corresponding to different
    /// compilation stages.
    struct Function
}

identifier! {
    /// A unique identifier for a function at a specific compile stage.
    struct StagedFunction
}

/// A specialized version of a function, identified by its function ID and specialization ID.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct SpecializedFunction(pub(crate) StagedFunction, pub(crate) usize);

impl SpecializedFunction {
    pub fn id(&self) -> (StagedFunction, usize) {
        (self.0, self.1)
    }
}

/// Information about a function across different compilation stages.
#[derive(Clone, Debug)]
pub struct FunctionInfo {
    id: Function,
    name: Option<Symbol>,
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
    pub fn new(id: Function, name: Option<Symbol>) -> Self {
        Self {
            id,
            name,
            staged_functions: HashMap::new(),
        }
    }

    pub fn name(&self) -> Option<&Symbol> {
        self.name.as_ref()
    }

    pub fn staged_functions(&self) -> &HashMap<CompileStage, StagedFunction> {
        &self.staged_functions
    }

    pub fn add_staged_function(&mut self, stage: CompileStage, func: StagedFunction) {
        self.staged_functions.insert(stage, func);
    }
}

#[derive(Clone, Debug)]
pub struct StagedFunctionInfo<L: Dialect> {
    pub(crate) id: StagedFunction,
    pub(crate) name: Option<Symbol>,
    pub(crate) signature: Signature<L>,
    pub(crate) return_type: L::TypeLattice,
    pub(crate) specializations: Vec<SpecializedFunctionInfo<L>>,
    /// Functions that call this staged function (used for inter-procedural analyses).
    /// note that the call statement must refer to the `StagedFunction` ID,
    /// if it refers to a `SpecializedFunction`, it should be recorded in the
    /// `backedges` field of `SpecializedFunctionInfo`.
    /// thus the `backedges` field of `SpecializedFunctionInfo` is always a superset of this field.
    pub(crate) backedges: Vec<StagedFunction>,
}

impl<L: Dialect> StagedFunctionInfo<L> {
    pub fn name(&self) -> Option<&Symbol> {
        self.name.as_ref()
    }

    pub fn signature(&self) -> &Signature<L> {
        &self.signature
    }

    pub fn return_type(&self) -> &L::TypeLattice {
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
pub struct SpecializedFunctionInfo<L: Dialect> {
    id: SpecializedFunction,
    signature: Signature<L>,
    return_type: L::TypeLattice,
    body: Statement,
    /// Functions that call this function (used for inter-procedural analyses).
    backedges: Vec<SpecializedFunction>,
}

#[bon::bon]
impl<L: Dialect> SpecializedFunctionInfo<L> {
    #[builder(finish_fn = new)]
    pub fn new(
        /// The unique identifier for this specialized function.
        id: SpecializedFunction,
        /// The signature of this specialized function.
        signature: Signature<L>,
        /// The return type of this specialized function.
        return_type: L::TypeLattice,
        /// The body of this specialized function.
        body: Statement,
        /// The functions that call this specialized function.
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> Self {
        Self {
            id,
            signature,
            return_type,
            body,
            backedges: backedges.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Signature<L: Dialect>(pub Vec<L::TypeLattice>);

impl<L: Dialect> PartialEq for Signature<L> {
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

impl<L: Dialect> PartialOrd for Signature<L> {
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

impl<L: Dialect> From<StagedFunctionInfo<L>> for StagedFunction {
    fn from(sfi: StagedFunctionInfo<L>) -> Self {
        sfi.id
    }
}

impl<L: Dialect> From<SpecializedFunctionInfo<L>> for SpecializedFunction {
    fn from(sfi: SpecializedFunctionInfo<L>) -> Self {
        sfi.id
    }
}

impl<L: Dialect> SpecializedFunctionInfo<L> {
    pub fn body(&self) -> &Statement {
        &self.body
    }

    pub fn body_mut(&mut self) -> &mut Statement {
        &mut self.body
    }

    pub fn return_type(&self) -> &L::TypeLattice {
        &self.return_type
    }

    pub fn signature(&self) -> &Signature<L> {
        &self.signature
    }

    pub fn backedges(&self) -> &Vec<SpecializedFunction> {
        &self.backedges
    }
}

impl<L: Dialect> Lattice for Signature<L> {
    fn join(&self, other: &Self) -> Self {
        if self.0.len() != other.0.len() {
            panic!("Cannot join signatures of different lengths");
        }
        let types = self
            .0
            .iter()
            .zip(other.0.iter())
            .map(|(a, b)| a.join(b))
            .collect();
        Signature(types)
    }

    fn meet(&self, other: &Self) -> Self {
        if self.0.len() != other.0.len() {
            panic!("Cannot meet signatures of different lengths");
        }
        let types = self
            .0
            .iter()
            .zip(other.0.iter())
            .map(|(a, b)| a.meet(b))
            .collect();
        Signature(types)
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            if !a.is_subseteq(b) {
                return false;
            }
        }
        true
    }
}

impl<L: Dialect> GetInfo<L> for StagedFunction {
    type Info = Item<StagedFunctionInfo<L>>;

    fn get_info<'a>(&self, context: &'a crate::Context<L>) -> Option<&'a Self::Info> {
        context.staged_functions.get(*self)
    }

    fn get_info_mut<'a>(&self, context: &'a mut crate::Context<L>) -> Option<&'a mut Self::Info> {
        context.staged_functions.get_mut(*self)
    }
}

impl<L: Dialect> GetInfo<L> for SpecializedFunction {
    type Info = SpecializedFunctionInfo<L>;

    fn get_info<'a>(&self, context: &'a crate::Context<L>) -> Option<&'a Self::Info> {
        let (staged_func, idx) = self.id();
        context
            .staged_functions
            .get(staged_func)
            .and_then(|f| f.specializations.get(idx))
    }

    fn get_info_mut<'a>(&self, context: &'a mut crate::Context<L>) -> Option<&'a mut Self::Info> {
        let (staged_func, idx) = self.id();
        context
            .staged_functions
            .get_mut(staged_func)
            .and_then(|f| f.specializations.get_mut(idx))
    }
}
