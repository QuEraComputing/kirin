use kirin_ir::{BuilderSSAInfo, BuilderSSAKind, BuilderStageInfo, Dialect};
use rustc_hash::FxHashMap;

/// Error type for IR emission from parsed AST nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub enum EmitError {
    /// An SSA value was referenced but never defined.
    UndefinedSSA(String),
    /// A block label was referenced but never defined.
    UndefinedBlock(String),
    /// An SSA name was defined more than once in the same scope.
    DuplicateSSA(String),
    /// A block name was defined more than once in the same scope.
    DuplicateBlock(String),
    /// A custom error from dialect-specific emit logic.
    Custom(String),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::UndefinedSSA(name) => write!(f, "undefined SSA value: %{name}"),
            EmitError::UndefinedBlock(name) => write!(f, "undefined block: ^{name}"),
            EmitError::DuplicateSSA(name) => {
                write!(f, "duplicate SSA definition in same scope: %{name}")
            }
            EmitError::DuplicateBlock(name) => {
                write!(f, "duplicate block definition in same scope: ^{name}")
            }
            EmitError::Custom(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EmitError {}

/// Type-erased function that creates a forward-reference SSA.
/// Stored in `EmitContext` so that `SSAValue::emit` doesn't need a `Placeholder` bound.
type ForwardRefCreator<L> = fn(&mut BuilderStageInfo<L>, &str) -> kirin_ir::SSAValue;

/// Context for emitting IR from parsed AST, tracking name mappings.
///
/// The `stage` field is a `&mut BuilderStageInfo<L>` since emit is a build-time
/// operation that needs access to builder methods (block, region, ssa, etc.).
pub struct EmitContext<'a, L: Dialect> {
    pub stage: &'a mut BuilderStageInfo<L>,
    /// Scope stack for SSA name bindings. Inner scopes shadow outer ones.
    /// The stack always has at least one entry (the root scope).
    ssa_scopes: Vec<FxHashMap<String, kirin_ir::SSAValue>>,
    /// Scope stack for block name bindings. Inner scopes shadow outer ones.
    /// The stack always has at least one entry (the root scope).
    block_scopes: Vec<FxHashMap<String, kirin_ir::Block>>,
    /// When set, undefined SSA references use this function to create
    /// forward-reference placeholders. Used for graph bodies with relaxed dominance.
    forward_ref_creator: Option<ForwardRefCreator<L>>,
    /// Function name extracted by `{:name}` context projection during parsing.
    /// The function text parser reads this after `parse_and_emit` completes.
    function_name: Option<String>,
}

impl<'a, L: Dialect> EmitContext<'a, L> {
    pub fn new(stage: &'a mut BuilderStageInfo<L>) -> Self {
        Self {
            stage,
            ssa_scopes: vec![FxHashMap::default()],
            block_scopes: vec![FxHashMap::default()],
            forward_ref_creator: None,
            function_name: None,
        }
    }

    /// Push a new scope onto both SSA and block scope stacks.
    ///
    /// Names registered after this call go into the new (inner) scope.
    /// Lookups search from innermost scope outward.
    pub fn push_scope(&mut self) {
        self.ssa_scopes.push(FxHashMap::default());
        self.block_scopes.push(FxHashMap::default());
    }

    /// Pop the innermost scope from both SSA and block scope stacks.
    ///
    /// All names registered in the popped scope are discarded, restoring
    /// visibility to any outer names that were shadowed.
    ///
    /// Panics if called when only the root scope remains.
    pub fn pop_scope(&mut self) {
        assert!(
            self.ssa_scopes.len() > 1,
            "cannot pop the root SSA scope"
        );
        assert!(
            self.block_scopes.len() > 1,
            "cannot pop the root block scope"
        );
        self.ssa_scopes.pop();
        self.block_scopes.pop();
    }

    /// Look up an SSA value by name, searching from innermost scope outward.
    pub fn lookup_ssa(&self, name: &str) -> Option<kirin_ir::SSAValue> {
        for scope in self.ssa_scopes.iter().rev() {
            if let Some(ssa) = scope.get(name) {
                return Some(*ssa);
            }
        }
        None
    }

    /// Look up an SSA value by name. If not found and a forward-reference
    /// creator is installed (relaxed dominance mode), creates a placeholder SSA
    /// in the current (top) scope.
    ///
    /// No `Placeholder` bound needed — the bound is captured in the creator
    /// function installed by [`set_relaxed_dominance`].
    pub fn resolve_ssa(&mut self, name: &str) -> Result<kirin_ir::SSAValue, EmitError> {
        if let Some(ssa) = self.lookup_ssa(name) {
            return Ok(ssa);
        }
        if let Some(creator) = self.forward_ref_creator {
            let ssa = creator(self.stage, name);
            self.ssa_scopes
                .last_mut()
                .expect("scope stack is never empty")
                .insert(name.to_string(), ssa);
            return Ok(ssa);
        }
        Err(EmitError::UndefinedSSA(name.to_string()))
    }

    /// Enable relaxed dominance mode: undefined SSA references create
    /// forward-reference `Unresolved(Result(0))` placeholders with `ty: None`.
    ///
    /// No `Placeholder` bound needed — forward refs use `Option<L::Type>` = `None`.
    pub fn set_relaxed_dominance(&mut self, relaxed: bool) {
        self.forward_ref_creator = if relaxed {
            Some(create_forward_ref::<L>)
        } else {
            None
        };
    }

    /// Register an SSA name in the current (top) scope.
    ///
    /// Returns an error if the name already exists in the current scope,
    /// enforcing the SSA single-assignment invariant within each scope level.
    pub fn register_ssa(
        &mut self,
        name: String,
        ssa: kirin_ir::SSAValue,
    ) -> Result<(), EmitError> {
        let top = self
            .ssa_scopes
            .last_mut()
            .expect("scope stack is never empty");
        if top.contains_key(&name) {
            return Err(EmitError::DuplicateSSA(name));
        }
        top.insert(name, ssa);
        Ok(())
    }

    /// Look up a block by name, searching from innermost scope outward.
    pub fn lookup_block(&self, name: &str) -> Option<kirin_ir::Block> {
        for scope in self.block_scopes.iter().rev() {
            if let Some(block) = scope.get(name) {
                return Some(*block);
            }
        }
        None
    }

    /// Register a block name in the current (top) scope.
    ///
    /// Returns an error if the name already exists in the current scope,
    /// enforcing block name uniqueness within each scope level.
    pub fn register_block(
        &mut self,
        name: String,
        block: kirin_ir::Block,
    ) -> Result<(), EmitError> {
        let top = self
            .block_scopes
            .last_mut()
            .expect("scope stack is never empty");
        if top.contains_key(&name) {
            return Err(EmitError::DuplicateBlock(name));
        }
        top.insert(name, block);
        Ok(())
    }

    /// Set the function name (called by `{:name}` parser codegen during emit).
    pub fn set_function_name(&mut self, name: String) {
        self.function_name = Some(name);
    }

    /// Get the function name that was parsed by `{:name}` during emit.
    /// Returns `None` if the format string doesn't include `{:name}`.
    pub fn function_name(&self) -> Option<&str> {
        self.function_name.as_deref()
    }
}

/// Create a forward-reference SSA for an undefined name in relaxed dominance mode.
///
/// No `Placeholder` bound needed — uses `ty: None` via direct `SSAInfo::new`.
fn create_forward_ref<L: Dialect>(
    stage: &mut BuilderStageInfo<L>,
    name: &str,
) -> kirin_ir::SSAValue {
    let symbol = stage.symbol_table_mut().intern(name.to_string());
    let ssas = stage.ssa_arena_mut();
    let id = ssas.next_id();
    let ssa = BuilderSSAInfo::new(
        id,
        Some(symbol),
        None,
        BuilderSSAKind::Unresolved(kirin_ir::ResolutionInfo::Result(0)),
    );
    let _ = ssas.alloc(ssa);
    id
}

/// Trait for emitting IR nodes from parsed AST nodes.
pub trait EmitIR<L: Dialect> {
    type Output;
    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Result<Self::Output, EmitError>;
}

/// Marker trait for types that can be directly parsed into themselves.
///
/// This is used to provide identity conversion for types that parse directly
/// into themselves (like type lattice types and compile-time values) without
/// running into coherence issues with blanket implementations.
pub trait DirectlyParsable: Clone {}

/// Blanket implementation of EmitIR for types that implement DirectlyParsable.
///
/// This allows types to emit to themselves (identity conversion),
/// which is useful for type lattices and compile-time value types.
impl<T, L> EmitIR<L> for T
where
    L: Dialect,
    T: DirectlyParsable,
{
    type Output = T;

    fn emit(&self, _ctx: &mut EmitContext<'_, L>) -> Result<Self::Output, EmitError> {
        Ok(self.clone())
    }
}

impl<T, L> EmitIR<L> for Vec<T>
where
    L: Dialect,
    T: EmitIR<L>,
{
    type Output = Vec<T::Output>;

    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Result<Self::Output, EmitError> {
        self.iter().map(|item| item.emit(ctx)).collect()
    }
}

impl<T, L> EmitIR<L> for Option<T>
where
    L: Dialect,
    T: EmitIR<L>,
{
    type Output = Option<T::Output>;

    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Result<Self::Output, EmitError> {
        self.as_ref().map(|item| item.emit(ctx)).transpose()
    }
}
