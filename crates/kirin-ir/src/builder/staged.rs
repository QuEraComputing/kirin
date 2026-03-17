use super::error::{SpecializeError, StagedFunctionConflictKind, StagedFunctionError};

use crate::Placeholder;
use crate::arena::GetInfo;
use crate::node::symbol::GlobalSymbol;
use crate::node::*;
use crate::signature::Signature;
use crate::{Dialect, StageInfo};

#[derive(Clone, Copy)]
enum PlaceholderKind {
    Port,
    Capture,
    BlockArgument,
}

/// Builder for creating placeholder SSA values that reference graph ports,
/// captures, or block arguments by index or name.
///
/// Created by [`StageInfo::graph_port`], [`StageInfo::graph_capture`],
/// or [`StageInfo::block_argument`]. Finalize with `.index(n)` or `.name("x")`.
pub struct PlaceholderBuilder<'a, L: Dialect> {
    stage: &'a mut StageInfo<L>,
    kind: PlaceholderKind,
}

impl<'a, L: Dialect> PlaceholderBuilder<'a, L>
where
    L::Type: Placeholder,
{
    fn make_ssa_kind(&self, key: BuilderKey) -> SSAKind {
        match self.kind {
            PlaceholderKind::Port => SSAKind::BuilderPort(key),
            PlaceholderKind::Capture => SSAKind::BuilderCapture(key),
            PlaceholderKind::BlockArgument => SSAKind::BuilderBlockArgument(key),
        }
    }

    /// Look up by positional index.
    pub fn index(self, index: usize) -> SSAValue {
        let kind = self.make_ssa_kind(BuilderKey::Index(index));
        let id = self.stage.ssas.next_id();
        let ssa = SSAInfo::new(id, None, L::Type::placeholder(), kind);
        self.stage.ssas.alloc(ssa);
        id
    }

    /// Look up by name (matched against the builder's `.port_name()` / `.arg_name()` / `.capture_name()` declarations).
    pub fn name(self, name: &str) -> SSAValue {
        let symbol = self.stage.symbols.intern(name.to_string());
        let kind = self.make_ssa_kind(BuilderKey::Named(symbol));
        let id = self.stage.ssas.next_id();
        let ssa = SSAInfo::new(id, None, L::Type::placeholder(), kind);
        self.stage.ssas.alloc(ssa);
        id
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

    /// Create a placeholder for a block argument, resolved when the block is built.
    pub fn block_argument(&mut self) -> PlaceholderBuilder<'_, L> {
        PlaceholderBuilder {
            stage: self,
            kind: PlaceholderKind::BlockArgument,
        }
    }

    /// Create a placeholder for a graph edge port, resolved when the graph is built.
    pub fn graph_port(&mut self) -> PlaceholderBuilder<'_, L> {
        PlaceholderBuilder {
            stage: self,
            kind: PlaceholderKind::Port,
        }
    }

    /// Create a placeholder for a graph capture, resolved when the graph is built.
    pub fn graph_capture(&mut self) -> PlaceholderBuilder<'_, L> {
        PlaceholderBuilder {
            stage: self,
            kind: PlaceholderKind::Capture,
        }
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
