use super::block::BlockBuilder;
use super::error::{SpecializeError, StagedFunctionConflictKind, StagedFunctionError};
use super::region::RegionBuilder;

use crate::arena::GetInfo;
use crate::node::symbol::GlobalSymbol;
use crate::node::*;
use crate::signature::Signature;
use crate::{Dialect, StageInfo};

impl<L: Dialect> StageInfo<L> {
    pub fn block(&mut self) -> BlockBuilder<'_, L> {
        BlockBuilder::from_stage(self)
    }

    pub fn region(&mut self) -> RegionBuilder<'_, L> {
        RegionBuilder::from_stage(self)
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
impl<L: Dialect> StageInfo<L> {
    #[builder(finish_fn = new)]
    pub fn ssa(
        &mut self,
        #[builder(into)] name: Option<String>,
        ty: L::Type,
        kind: SSAKind,
    ) -> SSAValue {
        let id = self.ssas.next_id();
        let ssa = SSAInfo::new(id.into(), name.map(|n| self.symbols.intern(n)), ty, kind);
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
    /// The `name` parameter accepts a [`GlobalSymbol`] pre-interned via
    /// [`Pipeline::intern`](crate::Pipeline::intern). This ensures function
    /// names are consistent across compilation stages.
    ///
    /// Returns `Err(StagedFunctionError)` when staged-function name policy would
    /// be violated:
    ///
    /// - `DuplicateSignature`: same name + same signature already exists.
    /// - `SignatureMismatchUnderSingleInterface`: same name + different signature
    ///   exists while [`StagedNamePolicy::SingleInterface`] is active.
    ///
    /// The error preserves all construction arguments so the caller can pass
    /// it to [`StageInfo::redefine_staged_function`] to intentionally overwrite
    /// the existing staged function.
    ///
    /// Anonymous staged functions (name = `None`) are never considered
    /// conflicting since they have no identity to collide on.
    #[builder(finish_fn = new)]
    pub fn staged_function(
        &mut self,
        name: Option<GlobalSymbol>,
        signature: Option<Signature<L::Type>>,
        specializations: Option<Vec<SpecializedFunctionInfo<L>>>,
        backedges: Option<Vec<StagedFunction>>,
    ) -> Result<StagedFunction, StagedFunctionError<L>> {
        let sig = signature.unwrap_or_default();

        // Check policy conflicts for named staged functions.
        if name.is_some() {
            let same_name: Vec<_> = self
                .staged_functions
                .iter()
                .filter(|item| {
                    let info: &StagedFunctionInfo<L> = item;
                    !info.invalidated && info.name == name
                })
                .map(|item| item.id)
                .collect();

            let duplicate_signature: Vec<_> = same_name
                .iter()
                .copied()
                .filter(|id| id.expect_info(self).signature() == &sig)
                .collect();

            if !duplicate_signature.is_empty() {
                return Err(StagedFunctionError {
                    conflict_kind: StagedFunctionConflictKind::DuplicateSignature,
                    name,
                    signature: sig,
                    conflicting: duplicate_signature,
                    specializations: specializations.unwrap_or_default(),
                    backedges: backedges.unwrap_or_default(),
                });
            }

            if self.staged_name_policy == StagedNamePolicy::SingleInterface {
                let signature_mismatch: Vec<_> = same_name
                    .into_iter()
                    .filter(|id| id.expect_info(self).signature() != &sig)
                    .collect();

                if !signature_mismatch.is_empty() {
                    return Err(StagedFunctionError {
                        conflict_kind:
                            StagedFunctionConflictKind::SignatureMismatchUnderSingleInterface,
                        name,
                        signature: sig,
                        conflicting: signature_mismatch,
                        specializations: specializations.unwrap_or_default(),
                        backedges: backedges.unwrap_or_default(),
                    });
                }
            }
        }

        let id = self.staged_functions.next_id();
        let staged_function = StagedFunctionInfo {
            id,
            name,
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
    /// arguments so the caller can pass it to [`StageInfo::redefine_specialization`]
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
/// These consume the error returned by [`StageInfo::specialize`] or
/// [`StageInfo::staged_function`] when a duplicate is detected, invalidate the
/// conflicting entries, and register the new definition.
impl<L: Dialect> StageInfo<L> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Block, Dialect, GlobalSymbol, HasArguments, HasArgumentsMut, HasBlocks, HasBlocksMut,
        HasRegions, HasRegionsMut, HasResults, HasResultsMut, HasSuccessors, HasSuccessorsMut,
        InternTable, IsConstant, IsPure, IsSpeculatable, IsTerminator, Region, ResultValue,
        SSAValue, StageInfo, StagedFunctionConflictKind, StagedNamePolicy, Successor,
    };

    #[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
    enum TestType {
        #[default]
        Any,
        I32,
        I64,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestDialect;

    impl<'a> HasArguments<'a> for TestDialect {
        type Iter = std::iter::Empty<&'a SSAValue>;

        fn arguments(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasArgumentsMut<'a> for TestDialect {
        type IterMut = std::iter::Empty<&'a mut SSAValue>;

        fn arguments_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasResults<'a> for TestDialect {
        type Iter = std::iter::Empty<&'a ResultValue>;

        fn results(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasResultsMut<'a> for TestDialect {
        type IterMut = std::iter::Empty<&'a mut ResultValue>;

        fn results_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasBlocks<'a> for TestDialect {
        type Iter = std::iter::Empty<&'a Block>;

        fn blocks(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasBlocksMut<'a> for TestDialect {
        type IterMut = std::iter::Empty<&'a mut Block>;

        fn blocks_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasSuccessors<'a> for TestDialect {
        type Iter = std::iter::Empty<&'a Successor>;

        fn successors(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasSuccessorsMut<'a> for TestDialect {
        type IterMut = std::iter::Empty<&'a mut Successor>;

        fn successors_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasRegions<'a> for TestDialect {
        type Iter = std::iter::Empty<&'a Region>;

        fn regions(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasRegionsMut<'a> for TestDialect {
        type IterMut = std::iter::Empty<&'a mut Region>;

        fn regions_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl IsTerminator for TestDialect {
        fn is_terminator(&self) -> bool {
            false
        }
    }

    impl IsConstant for TestDialect {
        fn is_constant(&self) -> bool {
            false
        }
    }

    impl IsPure for TestDialect {
        fn is_pure(&self) -> bool {
            true
        }
    }

    impl IsSpeculatable for TestDialect {
        fn is_speculatable(&self) -> bool {
            true
        }
    }

    impl Dialect for TestDialect {
        type Type = TestType;
    }

    fn sig(ty: TestType) -> Signature<TestType> {
        Signature {
            params: vec![ty.clone()],
            ret: ty,
            constraints: (),
        }
    }

    #[test]
    fn staged_name_policy_defaults_to_single_interface() {
        let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
        let foo = gs.intern("foo".to_string());
        let mut stage: StageInfo<TestDialect> = StageInfo::default();
        assert_eq!(
            stage.staged_name_policy(),
            StagedNamePolicy::SingleInterface
        );

        stage
            .staged_function()
            .name(foo)
            .signature(sig(TestType::I32))
            .new()
            .expect("first staged function should be created");

        let err = stage
            .staged_function()
            .name(foo)
            .signature(sig(TestType::I64))
            .new()
            .expect_err("same name + different signature should fail by default");

        assert_eq!(
            err.conflict_kind,
            StagedFunctionConflictKind::SignatureMismatchUnderSingleInterface
        );
    }

    #[test]
    fn staged_name_policy_multiple_dispatch_allows_different_signatures() {
        let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
        let foo = gs.intern("foo".to_string());
        let mut stage: StageInfo<TestDialect> = StageInfo::default();
        stage.set_staged_name_policy(StagedNamePolicy::MultipleDispatch);

        stage
            .staged_function()
            .name(foo)
            .signature(sig(TestType::I32))
            .new()
            .expect("first staged function should be created");

        stage
            .staged_function()
            .name(foo)
            .signature(sig(TestType::I64))
            .new()
            .expect("same name + different signature should be allowed under MultipleDispatch");
    }

    #[test]
    fn duplicate_signature_is_rejected_even_with_multiple_dispatch() {
        let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
        let foo = gs.intern("foo".to_string());
        let mut stage: StageInfo<TestDialect> = StageInfo::default();
        stage.set_staged_name_policy(StagedNamePolicy::MultipleDispatch);

        let i32_sig = sig(TestType::I32);
        stage
            .staged_function()
            .name(foo)
            .signature(i32_sig.clone())
            .new()
            .expect("first staged function should be created");

        let err = stage
            .staged_function()
            .name(foo)
            .signature(i32_sig)
            .new()
            .expect_err("duplicate (name, signature) should still fail");

        assert_eq!(
            err.conflict_kind,
            StagedFunctionConflictKind::DuplicateSignature
        );
    }
}
