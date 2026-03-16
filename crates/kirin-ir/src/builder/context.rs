use super::block::BlockBuilder;
use super::error::{SpecializeError, StagedFunctionConflictKind, StagedFunctionError};
use super::region::RegionBuilder;

use crate::Placeholder;
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
        let ssa = SSAInfo::new(id, name.map(|n| self.symbols.intern(n)), ty, kind);
        self.ssas.alloc(ssa);
        id
    }

    /// create a placeholder block argument SSAValue
    pub fn block_argument(&mut self, index: usize) -> BlockArgument
    where
        L::Type: crate::Placeholder,
    {
        let id: BlockArgument = self.ssas.next_id().into();
        let ssa = SSAInfo::new(
            id.into(),
            None,
            L::Type::placeholder(),
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
    ) -> Result<StagedFunction, StagedFunctionError<L>>
    where
        L::Type: crate::Placeholder,
    {
        let sig = signature.unwrap_or_else(|| Signature::placeholder());

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
    /// [`crate::SignatureSemantics::applicable`].
    #[builder(finish_fn = new)]
    pub fn specialize(
        &mut self,
        #[builder(name = staged_func)] func: StagedFunction,
        signature: Option<Signature<L::Type>>,
        #[builder(into)] body: Statement,
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> Result<SpecializedFunction, SpecializeError<L>> {
        let staged_function_info = func.expect_info_mut(self);

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
                staged_function: func,
                signature,
                conflicting,
                body,
                backedges,
            });
        }

        let id = SpecializedFunction(func, staged_function_info.specializations.len());

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
        Block, DiGraph, Dialect, GlobalSymbol, HasArguments, HasArgumentsMut, HasBlocks,
        HasBlocksMut, HasDigraphs, HasDigraphsMut, HasRegions, HasRegionsMut, HasResults,
        HasResultsMut, HasSuccessors, HasSuccessorsMut, HasUngraphs, HasUngraphsMut, InternTable,
        IsConstant, IsEdge, IsPure, IsSpeculatable, IsTerminator, Region, ResultValue, SSAValue,
        StageInfo, StagedFunctionConflictKind, StagedNamePolicy, Successor, UnGraph,
    };

    #[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
    enum TestType {
        #[default]
        Any,
        I32,
        I64,
    }

    impl crate::Placeholder for TestType {
        fn placeholder() -> Self {
            Self::Any
        }
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

    impl<'a> HasDigraphs<'a> for TestDialect {
        type Iter = std::iter::Empty<&'a DiGraph>;
        fn digraphs(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasDigraphsMut<'a> for TestDialect {
        type IterMut = std::iter::Empty<&'a mut DiGraph>;
        fn digraphs_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasUngraphs<'a> for TestDialect {
        type Iter = std::iter::Empty<&'a UnGraph>;
        fn ungraphs(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasUngraphsMut<'a> for TestDialect {
        type IterMut = std::iter::Empty<&'a mut UnGraph>;
        fn ungraphs_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl IsEdge for TestDialect {
        fn is_edge(&self) -> bool {
            false
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

    // --- Richer dialect supporting terminators for builder tests ---

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum RichDialect {
        Nop,
        Add(SSAValue, SSAValue),
        Return,
    }

    impl<'a> HasArguments<'a> for RichDialect {
        type Iter = std::vec::IntoIter<&'a SSAValue>;
        fn arguments(&'a self) -> Self::Iter {
            match self {
                RichDialect::Add(a, b) => vec![a, b].into_iter(),
                _ => vec![].into_iter(),
            }
        }
    }

    impl<'a> HasArgumentsMut<'a> for RichDialect {
        type IterMut = std::vec::IntoIter<&'a mut SSAValue>;
        fn arguments_mut(&'a mut self) -> Self::IterMut {
            match self {
                RichDialect::Add(a, b) => vec![a, b].into_iter(),
                _ => vec![].into_iter(),
            }
        }
    }

    impl<'a> HasResults<'a> for RichDialect {
        type Iter = std::iter::Empty<&'a ResultValue>;
        fn results(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasResultsMut<'a> for RichDialect {
        type IterMut = std::iter::Empty<&'a mut ResultValue>;
        fn results_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasBlocks<'a> for RichDialect {
        type Iter = std::iter::Empty<&'a Block>;
        fn blocks(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasBlocksMut<'a> for RichDialect {
        type IterMut = std::iter::Empty<&'a mut Block>;
        fn blocks_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasSuccessors<'a> for RichDialect {
        type Iter = std::iter::Empty<&'a Successor>;
        fn successors(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasSuccessorsMut<'a> for RichDialect {
        type IterMut = std::iter::Empty<&'a mut Successor>;
        fn successors_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasRegions<'a> for RichDialect {
        type Iter = std::iter::Empty<&'a Region>;
        fn regions(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasRegionsMut<'a> for RichDialect {
        type IterMut = std::iter::Empty<&'a mut Region>;
        fn regions_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl IsTerminator for RichDialect {
        fn is_terminator(&self) -> bool {
            matches!(self, RichDialect::Return)
        }
    }

    impl IsConstant for RichDialect {
        fn is_constant(&self) -> bool {
            false
        }
    }

    impl IsPure for RichDialect {
        fn is_pure(&self) -> bool {
            true
        }
    }

    impl IsSpeculatable for RichDialect {
        fn is_speculatable(&self) -> bool {
            true
        }
    }

    impl<'a> HasDigraphs<'a> for RichDialect {
        type Iter = std::iter::Empty<&'a DiGraph>;
        fn digraphs(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasDigraphsMut<'a> for RichDialect {
        type IterMut = std::iter::Empty<&'a mut DiGraph>;
        fn digraphs_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl<'a> HasUngraphs<'a> for RichDialect {
        type Iter = std::iter::Empty<&'a UnGraph>;
        fn ungraphs(&'a self) -> Self::Iter {
            std::iter::empty()
        }
    }

    impl<'a> HasUngraphsMut<'a> for RichDialect {
        type IterMut = std::iter::Empty<&'a mut UnGraph>;
        fn ungraphs_mut(&'a mut self) -> Self::IterMut {
            std::iter::empty()
        }
    }

    impl IsEdge for RichDialect {
        fn is_edge(&self) -> bool {
            false
        }
    }

    impl Dialect for RichDialect {
        type Type = TestType;
    }

    // --- BlockBuilder tests ---

    #[test]
    fn block_builder_creates_block_with_arguments_and_statements() {
        use crate::arena::GetInfo;
        use crate::node::SSAKind;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();

        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let s1 = stage.statement().definition(RichDialect::Nop).new();

        let block = stage
            .block()
            .argument(TestType::I32)
            .arg_name("x")
            .argument(TestType::I64)
            .arg_name("y")
            .stmt(s0)
            .stmt(s1)
            .new();

        let info = block.expect_info(&stage);
        assert_eq!(info.arguments.len(), 2);

        // Verify block arguments have correct SSAKind
        for (idx, &arg) in info.arguments.iter().enumerate() {
            let ssa = arg.expect_info(&stage);
            assert_eq!(ssa.kind, SSAKind::BlockArgument(block, idx));
        }

        // Verify iteration over statements
        let stmts: Vec<_> = block.statements(&stage).collect();
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], s0);
        assert_eq!(stmts[1], s1);
    }

    #[test]
    fn block_builder_substitutes_builder_block_arguments() {
        use crate::arena::GetInfo;
        use crate::node::SSAKind;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();

        // Create placeholder block arguments
        let arg0 = stage.block_argument(0);
        let arg1 = stage.block_argument(1);

        // Create a statement that uses the placeholder block arguments
        let add_stmt = stage
            .statement()
            .definition(RichDialect::Add(arg0.into(), arg1.into()))
            .new();

        let block = stage
            .block()
            .argument(TestType::I32)
            .argument(TestType::I64)
            .stmt(add_stmt)
            .new();

        let block_info = block.expect_info(&stage);
        let real_arg0: SSAValue = block_info.arguments[0].into();
        let real_arg1: SSAValue = block_info.arguments[1].into();

        // Verify the statement's arguments were substituted
        let stmt_info = add_stmt.expect_info(&stage);
        match &stmt_info.definition {
            RichDialect::Add(a, b) => {
                assert_eq!(*a, real_arg0, "first arg should be substituted");
                assert_eq!(*b, real_arg1, "second arg should be substituted");
            }
            _ => panic!("expected Add"),
        }

        // Verify the real block arguments have BlockArgument kind
        let ssa0 = real_arg0.get_info(&stage).unwrap();
        assert!(matches!(ssa0.kind, SSAKind::BlockArgument(_, 0)));
        let ssa1 = real_arg1.get_info(&stage).unwrap();
        assert!(matches!(ssa1.kind, SSAKind::BlockArgument(_, 1)));
    }

    #[test]
    #[should_panic(expected = "is not a terminator")]
    fn block_builder_terminator_rejects_non_terminator() {
        // DESIGN NOTE: BlockBuilder::terminator() panics instead of returning Result.
        // This is a footgun for users who might accidentally pass a non-terminator.
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let nop = stage.statement().definition(RichDialect::Nop).new();
        let _ = stage.block().terminator(nop).new();
    }

    #[test]
    #[should_panic(expected = "Cannot add terminator statement")]
    fn block_builder_stmt_rejects_terminator() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let ret = stage.statement().definition(RichDialect::Return).new();
        let _ = stage.block().stmt(ret).new();
    }

    // --- StatementIter tests ---

    #[test]
    fn statement_iter_double_ended() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let s1 = stage.statement().definition(RichDialect::Nop).new();
        let s2 = stage.statement().definition(RichDialect::Nop).new();

        let block = stage.block().stmt(s0).stmt(s1).stmt(s2).new();

        // Collect via next_back (reverse)
        let mut iter = block.statements(&stage);
        let last = iter.next_back().unwrap();
        let mid = iter.next_back().unwrap();
        let first = iter.next_back().unwrap();
        assert_eq!(first, s0);
        assert_eq!(mid, s1);
        assert_eq!(last, s2);
        assert!(iter.next_back().is_none());
    }

    #[test]
    fn statement_iter_exact_size() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let s1 = stage.statement().definition(RichDialect::Nop).new();

        let block = stage.block().stmt(s0).stmt(s1).new();

        let mut iter = block.statements(&stage);
        assert_eq!(iter.len(), 2);
        iter.next();
        assert_eq!(iter.len(), 1);
        iter.next();
        assert_eq!(iter.len(), 0);
    }

    #[test]
    fn block_first_last_statement_with_terminator_only() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let ret = stage.statement().definition(RichDialect::Return).new();
        let block = stage.block().terminator(ret).new();

        // No non-terminator statements
        assert_eq!(block.statements(&stage).len(), 0);
        // first_statement falls back to terminator
        assert_eq!(block.first_statement(&stage), Some(ret));
        // last_statement returns terminator
        assert_eq!(block.last_statement(&stage), Some(ret));
    }

    #[test]
    fn block_last_statement_without_terminator() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let s1 = stage.statement().definition(RichDialect::Nop).new();
        let block = stage.block().stmt(s0).stmt(s1).new();

        // No terminator — last_statement should be tail of linked list
        assert_eq!(block.terminator(&stage), None);
        assert_eq!(block.last_statement(&stage), Some(s1));
        assert_eq!(block.first_statement(&stage), Some(s0));
    }

    // --- RegionBuilder tests ---

    #[test]
    fn region_builder_creates_region_with_ordered_blocks() {
        use crate::arena::GetInfo;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let b0 = stage.block().new();
        let b1 = stage.block().new();
        let b2 = stage.block().new();

        let region = stage
            .region()
            .add_block(b0)
            .add_block(b1)
            .add_block(b2)
            .new();

        let info = region.expect_info(&stage);
        assert_eq!(info.blocks.len, 3);
        assert_eq!(info.blocks.head, Some(b0));
        assert_eq!(info.blocks.tail, Some(b2));

        // Verify linked list order via block nodes
        let b0_info = b0.expect_info(&stage);
        assert_eq!(b0_info.node.next, Some(b1));
        let b1_info = b1.expect_info(&stage);
        assert_eq!(b1_info.node.prev, Some(b0));
        assert_eq!(b1_info.node.next, Some(b2));
        let b2_info = b2.expect_info(&stage);
        assert_eq!(b2_info.node.prev, Some(b1));
        assert_eq!(b2_info.node.next, None);
    }

    #[test]
    #[should_panic(expected = "already added to the region")]
    fn region_builder_panics_on_duplicate_block() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let b0 = stage.block().new();
        let _ = stage.region().add_block(b0).add_block(b0).new();
    }

    // --- Detach tests ---

    #[test]
    fn detach_statement_updates_neighbors_and_parent_len() {
        use crate::arena::GetInfo;
        use crate::detach::Detach;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let s1 = stage.statement().definition(RichDialect::Nop).new();
        let s2 = stage.statement().definition(RichDialect::Nop).new();
        let block = stage.block().stmt(s0).stmt(s1).stmt(s2).new();

        // Detach the middle statement
        s1.detach(&mut stage);

        // Parent block should have 2 statements
        let block_info = block.expect_info(&stage);
        assert_eq!(block_info.statements.len, 2);

        // s0 and s2 should be neighbors
        let s0_info = s0.expect_info(&stage);
        assert_eq!(s0_info.node.next, Some(s2));
        let s2_info = s2.expect_info(&stage);
        assert_eq!(s2_info.node.prev, Some(s0));

        // Detached statement should have no links
        let s1_info = s1.expect_info(&stage);
        assert_eq!(s1_info.node.prev, None);
        assert_eq!(s1_info.node.next, None);
        assert_eq!(s1_info.parent, None);
    }

    #[test]
    fn detach_head_statement_updates_block_head() {
        use crate::arena::GetInfo;
        use crate::detach::Detach;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let s1 = stage.statement().definition(RichDialect::Nop).new();
        let block = stage.block().stmt(s0).stmt(s1).new();

        s0.detach(&mut stage);

        let block_info = block.expect_info(&stage);
        assert_eq!(block_info.statements.head, Some(s1));
        assert_eq!(block_info.statements.len, 1);
    }

    #[test]
    fn detach_tail_statement_updates_block_tail() {
        use crate::arena::GetInfo;
        use crate::detach::Detach;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let s1 = stage.statement().definition(RichDialect::Nop).new();
        let block = stage.block().stmt(s0).stmt(s1).new();

        s1.detach(&mut stage);

        let block_info = block.expect_info(&stage);
        assert_eq!(block_info.statements.tail, Some(s0));
        assert_eq!(block_info.statements.len, 1);
    }

    // --- Specialize / Redefine tests ---

    #[test]
    fn specialize_success_and_duplicate_error() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();

        let sf = stage.staged_function().new().unwrap();

        let body1 = stage.statement().definition(RichDialect::Return).new();
        let _spec1 = stage
            .specialize()
            .staged_func(sf)
            .body(body1)
            .new()
            .expect("first specialize should succeed");

        let body2 = stage.statement().definition(RichDialect::Return).new();
        let err = stage
            .specialize()
            .staged_func(sf)
            .body(body2)
            .new()
            .expect_err("duplicate signature should fail");

        assert_eq!(err.conflicting.len(), 1);
    }

    #[test]
    fn redefine_specialization_invalidates_and_registers() {
        use crate::arena::GetInfo;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let sf = stage.staged_function().new().unwrap();

        let body1 = stage.statement().definition(RichDialect::Return).new();
        let spec1 = stage
            .specialize()
            .staged_func(sf)
            .body(body1)
            .new()
            .unwrap();

        let body2 = stage.statement().definition(RichDialect::Return).new();
        let err = stage
            .specialize()
            .staged_func(sf)
            .body(body2)
            .new()
            .expect_err("duplicate");

        let spec2 = stage.redefine_specialization(err);

        // Old specialization should be invalidated
        let old = spec1.get_info(&stage).unwrap();
        assert!(old.is_invalidated());

        // New specialization should be valid
        let new = spec2.get_info(&stage).unwrap();
        assert!(!new.is_invalidated());
    }

    #[test]
    fn redefine_staged_function_invalidates_and_registers() {
        use crate::arena::GetInfo;

        let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
        let foo = gs.intern("foo".to_string());
        let mut stage: StageInfo<RichDialect> = StageInfo::default();

        let sf1 = stage.staged_function().name(foo).new().unwrap();

        let err = stage
            .staged_function()
            .name(foo)
            .new()
            .expect_err("duplicate signature");

        let sf2 = stage.redefine_staged_function(err);

        let old_info = sf1.get_info(&stage).unwrap();
        assert!(old_info.is_invalidated());

        let new_info = sf2.get_info(&stage).unwrap();
        assert!(!new_info.is_invalidated());
    }

    // --- remap_block_identity tests ---

    #[test]
    fn remap_block_identity_remaps_parents_and_ssa_kinds() {
        use crate::arena::GetInfo;
        use crate::node::SSAKind;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();

        // Create a stub block (empty, pre-allocated for forward refs)
        let stub = stage.block().new();

        // Create a real block with content
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let real = stage.block().argument(TestType::I32).stmt(s0).new();

        // Remap real -> stub
        stage.remap_block_identity(stub, real);

        // Statement parent should now point to stub
        let stmt_info = s0.expect_info(&stage);
        assert_eq!(
            stmt_info.parent,
            Some(crate::node::stmt::StatementParent::Block(stub))
        );

        // Block arguments should have SSAKind pointing to stub
        let stub_info = stub.expect_info(&stage);
        assert_eq!(stub_info.arguments.len(), 1);
        let arg = stub_info.arguments[0];
        let arg_info = arg.expect_info(&stage);
        assert!(matches!(arg_info.kind, SSAKind::BlockArgument(owner, 0) if owner == stub));

        // Real block should be deleted
        assert!(stage.blocks.get(real).unwrap().deleted());
    }

    // --- StagedFunctionInfo::all_matching tests ---

    #[test]
    fn staged_function_all_matching_returns_most_specific() {
        use crate::arena::GetInfo;
        use crate::signature::{ExactSemantics, Signature};

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let sf = stage.staged_function().new().unwrap();

        // Create two specializations with different signatures
        let body1 = stage.statement().definition(RichDialect::Return).new();
        let sig_i32 = Signature {
            params: vec![TestType::I32],
            ret: TestType::Any,
            constraints: (),
        };
        let _spec1 = stage
            .specialize()
            .staged_func(sf)
            .signature(sig_i32.clone())
            .body(body1)
            .new()
            .unwrap();

        let body2 = stage.statement().definition(RichDialect::Return).new();
        let sig_i64 = Signature {
            params: vec![TestType::I64],
            ret: TestType::Any,
            constraints: (),
        };
        let _spec2 = stage
            .specialize()
            .staged_func(sf)
            .signature(sig_i64)
            .body(body2)
            .new()
            .unwrap();

        let sf_info = sf.get_info(&stage).unwrap();

        // Query with I32 signature — should match only spec1
        let matches = sf_info.all_matching::<ExactSemantics>(&sig_i32);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0.signature(), &sig_i32);
    }

    #[test]
    fn staged_function_all_matching_excludes_invalidated() {
        use crate::arena::GetInfo;
        use crate::signature::{ExactSemantics, Signature};

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let sf = stage.staged_function().new().unwrap();

        let body1 = stage.statement().definition(RichDialect::Return).new();
        let default_sig: Signature<TestType> = Signature::placeholder();
        let spec1 = stage
            .specialize()
            .staged_func(sf)
            .body(body1)
            .new()
            .unwrap();

        // Invalidate spec1 by redefining
        let body2 = stage.statement().definition(RichDialect::Return).new();
        let err = stage
            .specialize()
            .staged_func(sf)
            .body(body2)
            .new()
            .expect_err("duplicate");
        let _spec2 = stage.redefine_specialization(err);

        let sf_info = sf.get_info(&stage).unwrap();
        let matches = sf_info.all_matching::<ExactSemantics>(&default_sig);

        // Only the new (non-invalidated) spec should match
        assert_eq!(matches.len(), 1);

        // Verify the old one is indeed invalidated
        let old = spec1.get_info(&stage).unwrap();
        assert!(old.is_invalidated());
    }

    // --- link_statements edge cases ---

    #[test]
    fn link_statements_empty_slice() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let list = stage.link_statements(&[]);
        assert_eq!(list.len(), 0);
        assert!(list.head().is_none());
        assert!(list.tail().is_none());
    }

    #[test]
    fn link_statements_single_element() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let list = stage.link_statements(&[s0]);
        assert_eq!(list.len(), 1);
        assert_eq!(list.head(), Some(&s0));
        assert_eq!(list.tail(), Some(&s0));
        // Single element should have no prev/next
        let info = s0.expect_info(&stage);
        assert_eq!(info.node.prev, None);
        assert_eq!(info.node.next, None);
    }

    // --- link_blocks edge cases ---

    #[test]
    fn link_blocks_empty_slice() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let list = stage.link_blocks(&[]);
        assert_eq!(list.len(), 0);
        assert!(list.head().is_none());
        assert!(list.tail().is_none());
    }

    #[test]
    fn link_blocks_single_element() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let b0 = stage.block().new();
        let list = stage.link_blocks(&[b0]);
        assert_eq!(list.len(), 1);
        assert_eq!(list.head(), Some(&b0));
        assert_eq!(list.tail(), Some(&b0));
    }

    // --- Empty block iteration ---

    #[test]
    fn empty_block_iteration() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let block = stage.block().new();

        let stmts: Vec<_> = block.statements(&stage).collect();
        assert!(stmts.is_empty());
        assert_eq!(block.statements(&stage).len(), 0);
        assert_eq!(block.first_statement(&stage), None);
        assert_eq!(block.last_statement(&stage), None);
        assert_eq!(block.terminator(&stage), None);
    }

    // --- Single statement double-ended iteration ---

    #[test]
    fn single_statement_double_ended_iteration() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let block = stage.block().stmt(s0).new();

        // Forward
        let mut iter = block.statements(&stage);
        assert_eq!(iter.next(), Some(s0));
        assert_eq!(iter.next(), None);

        // Backward
        let mut iter = block.statements(&stage);
        assert_eq!(iter.next_back(), Some(s0));
        assert_eq!(iter.next_back(), None);
    }

    // --- Region BlockIter ---

    #[test]
    fn region_block_iter_single_block() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let b0 = stage.block().new();
        let region = stage.region().add_block(b0).new();

        let blocks: Vec<_> = region.blocks(&stage).collect();
        assert_eq!(blocks, vec![b0]);
        assert_eq!(region.blocks(&stage).len(), 1);
    }

    #[test]
    fn region_block_iter_double_ended() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let b0 = stage.block().new();
        let b1 = stage.block().new();
        let b2 = stage.block().new();
        let region = stage
            .region()
            .add_block(b0)
            .add_block(b1)
            .add_block(b2)
            .new();

        let mut iter = region.blocks(&stage);
        assert_eq!(iter.next_back(), Some(b2));
        assert_eq!(iter.next(), Some(b0));
        assert_eq!(iter.next_back(), Some(b1));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn region_block_iter_exact_size() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let b0 = stage.block().new();
        let b1 = stage.block().new();
        let region = stage.region().add_block(b0).add_block(b1).new();

        let mut iter = region.blocks(&stage);
        assert_eq!(iter.len(), 2);
        iter.next();
        assert_eq!(iter.len(), 1);
        iter.next();
        assert_eq!(iter.len(), 0);
    }

    // --- Empty region ---

    #[test]
    fn empty_region() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let region = stage.region().new();

        let blocks: Vec<_> = region.blocks(&stage).collect();
        assert!(blocks.is_empty());
        assert_eq!(region.blocks(&stage).len(), 0);
    }

    // --- SSA creation edge cases ---

    #[test]
    fn ssa_with_name_is_resolvable() {
        use crate::node::SSAKind;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let ssa = stage
            .ssa()
            .name("x")
            .ty(TestType::I32)
            .kind(SSAKind::Test)
            .new();

        let info = ssa.expect_info(&stage);
        assert!(info.name().is_some());
        assert_eq!(info.ty(), &TestType::I32);
        assert_eq!(*info.kind(), SSAKind::Test);
    }

    #[test]
    fn ssa_without_name() {
        use crate::node::SSAKind;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let ssa = stage.ssa().ty(TestType::I64).kind(SSAKind::Test).new();

        let info = ssa.expect_info(&stage);
        assert!(info.name().is_none());
        assert_eq!(info.ty(), &TestType::I64);
    }

    // --- Block argument edge cases ---

    #[test]
    fn block_argument_placeholder_substitution_with_zero_args() {
        // Block with no arguments — should work fine
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let block = stage.block().stmt(s0).new();

        let info = block.expect_info(&stage);
        assert!(info.arguments.is_empty());
    }

    // --- Detach edge cases ---

    #[test]
    fn detach_only_statement_leaves_empty_block() {
        use crate::arena::GetInfo;
        use crate::detach::Detach;

        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let block = stage.block().stmt(s0).new();

        s0.detach(&mut stage);

        let block_info = block.expect_info(&stage);
        assert_eq!(block_info.statements.len, 0);
        assert_eq!(block_info.statements.head, None);
        assert_eq!(block_info.statements.tail, None);
    }

    // --- Statement with terminator + body statements ---

    #[test]
    fn block_with_statements_and_terminator() {
        let mut stage: StageInfo<RichDialect> = StageInfo::default();
        let s0 = stage.statement().definition(RichDialect::Nop).new();
        let s1 = stage.statement().definition(RichDialect::Nop).new();
        let ret = stage.statement().definition(RichDialect::Return).new();

        let block = stage.block().stmt(s0).stmt(s1).terminator(ret).new();

        // statements() should only iterate non-terminator statements
        let stmts: Vec<_> = block.statements(&stage).collect();
        assert_eq!(stmts, vec![s0, s1]);

        // terminator should be ret
        assert_eq!(block.terminator(&stage), Some(ret));

        // first_statement is head of linked list
        assert_eq!(block.first_statement(&stage), Some(s0));

        // last_statement is the terminator
        assert_eq!(block.last_statement(&stage), Some(ret));
    }
}
