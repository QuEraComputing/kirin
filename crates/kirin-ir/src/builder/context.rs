use super::block::BlockBuilder;
use super::error::{SpecializeError, StagedFunctionError};
use super::region::RegionBuilder;

use crate::arena::GetInfo;
use crate::node::*;
use crate::signature::Signature;
use crate::{Context, Dialect};

impl<L: Dialect> Context<L> {
    pub fn block(&mut self) -> BlockBuilder<'_, L> {
        BlockBuilder::from_context(self)
    }

    pub fn region(&mut self) -> RegionBuilder<'_, L> {
        RegionBuilder::from_context(self)
    }

    pub fn link_statements(&mut self, ptrs: &[Statement]) -> LinkedList<Statement> {
        for window in ptrs.windows(2) {
            let current = window[0];
            let next = window[1];
            let current_stmt = current.expect_info_mut(self);
            if let Some(next) = current_stmt.node.next {
                let info = next.expect_info(self);
                panic!("Statement already has a next node: {:?}", info.definition);
            }
            current_stmt.node.next = Some(next);

            let next_stmt = next.expect_info_mut(self);
            if let Some(prev) = next_stmt.node.prev {
                let info = prev.expect_info(self);
                panic!(
                    "Statement already has a previous node: {:?}",
                    info.definition
                );
            }
            next_stmt.node.prev = Some(current);
        }
        LinkedList {
            head: ptrs.first().copied(),
            tail: ptrs.last().copied(),
            len: ptrs.len(),
        }
    }

    pub fn link_blocks(&mut self, ptrs: &[Block]) -> LinkedList<Block> {
        for window in ptrs.windows(2) {
            let current = window[0];
            let next = window[1];
            let current_block = current.expect_info_mut(self);
            if let Some(next) = current_block.node.next {
                let info = next.expect_info(self);
                panic!("Block already has a next node: {:?}", info);
            }
            current_block.node.next = Some(next);

            let next_block = next.expect_info_mut(self);
            if let Some(prev) = next_block.node.prev {
                let info = prev.expect_info(self);
                panic!("Block already has a previous node: {:?}", info);
            }
            next_block.node.prev = Some(current);
        }
        LinkedList {
            head: ptrs.first().copied(),
            tail: ptrs.last().copied(),
            len: ptrs.len(),
        }
    }
}

#[bon::bon]
impl<L: Dialect> Context<L> {
    #[builder(finish_fn = new)]
    pub fn ssa(
        &mut self,
        #[builder(into)] name: Option<String>,
        ty: L::Type,
        kind: SSAKind,
    ) -> SSAValue {
        let id = self.ssas.next_id();
        let ssa = SSAInfo::new(
            id.into(),
            name.map(|n| self.symbols.borrow_mut().intern(n)),
            ty,
            kind,
        );
        self.ssas.alloc(ssa);
        id
    }

    /// create a placeholder block argument SSAValue
    pub fn block_argument(&mut self, index: usize) -> BlockArgument {
        let id: BlockArgument = self.ssas.next_id().into();
        let ssa = SSAInfo::new(
            id.into(),
            None,
            L::Type::default(),
            SSAKind::BuilderBlockArgument(index),
        );
        self.ssas.alloc(ssa);
        id
    }

    #[builder(finish_fn = new)]
    pub fn statement(&mut self, #[builder(into)] definition: L) -> Statement {
        let id = self.statements.next_id();
        let statement = StatementInfo {
            node: LinkedListNode::new(id),
            parent: None,
            definition,
        };
        self.statements.alloc(statement);
        id
    }

    /// Create a new staged function.
    ///
    /// Returns `Err(StagedFunctionError)` if a non-invalidated staged function
    /// with the same (name, signature) already exists in the arena. The error
    /// preserves all construction arguments so the caller can pass it to
    /// [`Context::redefine_staged_function`] to intentionally overwrite the
    /// existing staged function.
    #[builder(finish_fn = new)]
    pub fn staged_function(
        &mut self,
        #[builder(into)] name: Option<String>,
        signature: Option<Signature<L::Type>>,
        specializations: Option<Vec<SpecializedFunctionInfo<L>>>,
        backedges: Option<Vec<StagedFunction>>,
    ) -> Result<StagedFunction, StagedFunctionError<L>> {
        let interned_name = name.map(|n| self.symbols.borrow_mut().intern(n));
        let sig = signature.unwrap_or_default();

        // Check for existing non-invalidated staged functions with the same (name, signature)
        let conflicting: Vec<StagedFunction> = self
            .staged_functions
            .iter()
            .filter(|item| {
                let info: &StagedFunctionInfo<L> = item;
                !info.invalidated && info.name == interned_name && info.signature == sig
            })
            .map(|item| item.id)
            .collect();

        if !conflicting.is_empty() {
            return Err(StagedFunctionError {
                name: interned_name,
                signature: sig,
                conflicting,
                specializations: specializations.unwrap_or_default(),
                backedges: backedges.unwrap_or_default(),
            });
        }

        let id = self.staged_functions.next_id();
        let staged_function = StagedFunctionInfo {
            id,
            name: interned_name,
            signature: sig,
            specializations: specializations.unwrap_or_default(),
            backedges: backedges.unwrap_or_default(),
            invalidated: false,
        };
        self.staged_functions.alloc(staged_function);
        Ok(id)
    }

    /// Create a specialized function from a staged function.
    ///
    /// Returns `Err(SpecializeError)` if a non-invalidated specialization with
    /// the same signature already exists. The error preserves all construction
    /// arguments so the caller can pass it to [`Context::redefine_specialization`]
    /// to intentionally overwrite the existing specialization.
    ///
    /// # Design: Signature ownership
    ///
    /// Signatures are explicitly provided here rather than derived from the body
    /// statement. This is intentional:
    ///
    /// - **StagedFunction** owns the user-declared signature from the frontend.
    /// - **SpecializedFunction** owns the compiler-derived signature (a subset of
    ///   staged). If not provided, it defaults to the staged function's signature.
    /// - **Body statement** is a structural container (Region + dialect-specific
    ///   context) and does not encode signature information. A Region's block
    ///   arguments have dialect-specific semantics and do not universally
    ///   correspond to function parameters.
    /// - **Extern functions** are represented as StagedFunctions with no
    ///   specializations (empty `specializations` vec).
    ///
    /// Signature validation (e.g., checking that the specialized signature is a
    /// subset of the staged signature) is the caller's responsibility via
    /// [`SignatureSemantics::applicable`].
    #[builder(finish_fn = new)]
    pub fn specialize(
        &mut self,
        f: StagedFunction,
        signature: Option<Signature<L::Type>>,
        #[builder(into)] body: Statement,
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> Result<SpecializedFunction, SpecializeError<L>> {
        let staged_function_info = f.expect_info_mut(self);

        let signature = signature.unwrap_or(staged_function_info.signature.clone());

        // Check for existing non-invalidated specializations with the same signature
        let conflicting: Vec<SpecializedFunction> = staged_function_info
            .specializations
            .iter()
            .filter(|s| !s.is_invalidated() && s.signature() == &signature)
            .map(|s| s.id())
            .collect();

        if !conflicting.is_empty() {
            return Err(SpecializeError {
                staged_function: f,
                signature,
                conflicting,
                body,
                backedges,
            });
        }

        let id = SpecializedFunction(f, staged_function_info.specializations.len());

        let specialized_function = SpecializedFunctionInfo::builder()
            .id(id)
            .signature(signature)
            .body(body)
            .maybe_backedges(backedges)
            .new();
        staged_function_info
            .specializations
            .push(specialized_function);
        Ok(id)
    }
}

/// Methods for intentionally redefining (overwriting) existing functions.
///
/// These consume the error returned by [`Context::specialize`] or
/// [`Context::staged_function`] when a duplicate is detected, invalidate the
/// conflicting entries, and register the new definition.
impl<L: Dialect> Context<L> {
    /// Redefine a specialization by consuming a [`SpecializeError`].
    ///
    /// Invalidates all conflicting specializations identified in the error
    /// and registers the new specialization. Returns the new
    /// [`SpecializedFunction`] ID.
    ///
    /// Callers should inspect the [`SpecializeError::conflicting`] backedges
    /// to determine what needs recompilation.
    pub fn redefine_specialization(&mut self, error: SpecializeError<L>) -> SpecializedFunction {
        let staged_function_info = error.staged_function.expect_info_mut(self);

        // Invalidate all conflicting specializations
        for conflict in &error.conflicting {
            let (_, idx) = conflict.id();
            staged_function_info.specializations[idx].invalidate();
        }

        // Push the new specialization
        let id = SpecializedFunction(
            error.staged_function,
            staged_function_info.specializations.len(),
        );
        let specialized_function = SpecializedFunctionInfo::builder()
            .id(id)
            .signature(error.signature)
            .body(error.body)
            .maybe_backedges(error.backedges)
            .new();
        staged_function_info
            .specializations
            .push(specialized_function);
        id
    }

    /// Redefine a staged function by consuming a [`StagedFunctionError`].
    ///
    /// Invalidates all conflicting staged functions identified in the error
    /// and registers the new staged function. Returns the new
    /// [`StagedFunction`] ID.
    ///
    /// Callers should inspect the backedges of the conflicting staged
    /// functions to determine what needs recompilation.
    pub fn redefine_staged_function(&mut self, error: StagedFunctionError<L>) -> StagedFunction {
        // Invalidate all conflicting staged functions
        for &conflict in &error.conflicting {
            let info = conflict.expect_info_mut(self);
            info.invalidate();
        }

        // Allocate the new staged function
        let id = self.staged_functions.next_id();
        let staged_function = StagedFunctionInfo {
            id,
            name: error.name,
            signature: error.signature,
            specializations: error.specializations,
            backedges: error.backedges,
            invalidated: false,
        };
        self.staged_functions.alloc(staged_function);
        id
    }
}
