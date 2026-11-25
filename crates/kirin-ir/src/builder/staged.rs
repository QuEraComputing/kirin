use super::error::{
    DuplicateSpecialization, InterfaceMismatch, SpecializeError, StagedFunctionConflictKind,
    StagedFunctionError,
};
use super::stage_info::BuilderStageInfo;

use crate::Dialect;
use crate::node::symbol::GlobalSymbol;
use crate::node::*;
use crate::signature::{HasSignature, Signature, SignatureSemantics};

#[derive(Clone, Copy)]
enum PlaceholderKind {
    Port,
    Capture,
    BlockArgument,
}

/// Builder for creating placeholder SSA values that reference graph ports,
/// captures, or block arguments by index or name.
///
/// Created by [`BuilderStageInfo::graph_port`], [`BuilderStageInfo::graph_capture`],
/// or [`BuilderStageInfo::block_argument`]. Finalize with `.index(n)` or `.name("x")`.
pub struct PlaceholderBuilder<'a, L: Dialect> {
    stage: &'a mut BuilderStageInfo<L>,
    kind: PlaceholderKind,
}

impl<'a, L: Dialect> PlaceholderBuilder<'a, L> {
    fn make_ssa_kind(&self, key: BuilderKey) -> BuilderSSAKind {
        let info = match self.kind {
            PlaceholderKind::Port => ResolutionInfo::Port(key),
            PlaceholderKind::Capture => ResolutionInfo::Capture(key),
            PlaceholderKind::BlockArgument => ResolutionInfo::BlockArgument(key),
        };
        BuilderSSAKind::Unresolved(info)
    }

    /// Look up by positional index.
    pub fn index(self, index: usize) -> SSAValue {
        let kind = self.make_ssa_kind(BuilderKey::Index(index));
        let id = self.stage.ssas.next_id();
        let ssa = BuilderSSAInfo::new(id, None, None, kind);
        let _ = self.stage.ssas.alloc(ssa);
        id
    }

    /// Look up by name (matched against the builder's `.port_name()` / `.arg_name()` / `.capture_name()` declarations).
    pub fn name(self, name: &str) -> SSAValue {
        let symbol = self.stage.symbols.intern(name.to_string());
        let kind = self.make_ssa_kind(BuilderKey::Named(symbol));
        let id = self.stage.ssas.next_id();
        let ssa = BuilderSSAInfo::new(id, None, None, kind);
        let _ = self.stage.ssas.alloc(ssa);
        id
    }
}

#[bon::bon]
impl<L: Dialect> BuilderStageInfo<L> {
    /// Create a new SSA value with a type and kind.
    ///
    /// Usually created implicitly by block/graph builders. Direct use is for
    /// test SSAs or pre-allocated results before their parent statement exists.
    #[builder(finish_fn = new)]
    pub fn ssa(
        &mut self,
        #[builder(into)] name: Option<String>,
        ty: L::Type,
        kind: BuilderSSAKind,
    ) -> SSAValue {
        let id = self.ssas.next_id();
        let ssa = BuilderSSAInfo::new(id, name.map(|n| self.symbols.intern(n)), Some(ty), kind);
        let _ = self.ssas.alloc(ssa);
        id
    }

    /// Create a placeholder for a block argument, resolved when the block is built.
    ///
    /// A placeholder is single-use: building the block that uses it deletes it,
    /// so using the same placeholder in a second block panics. Create a fresh
    /// one per block.
    pub fn block_argument(&mut self) -> PlaceholderBuilder<'_, L> {
        PlaceholderBuilder {
            stage: self,
            kind: PlaceholderKind::BlockArgument,
        }
    }

    /// Create a placeholder for a graph edge port, resolved when the graph is built.
    ///
    /// Single-use, like [`block_argument`](Self::block_argument): create a fresh
    /// one per graph.
    pub fn graph_port(&mut self) -> PlaceholderBuilder<'_, L> {
        PlaceholderBuilder {
            stage: self,
            kind: PlaceholderKind::Port,
        }
    }

    /// Create a placeholder for a graph capture, resolved when the graph is built.
    ///
    /// Single-use, like [`block_argument`](Self::block_argument): create a fresh
    /// one per graph.
    pub fn graph_capture(&mut self) -> PlaceholderBuilder<'_, L> {
        PlaceholderBuilder {
            stage: self,
            kind: PlaceholderKind::Capture,
        }
    }

    /// Create a statement from a dialect definition.
    ///
    /// Any `ResultValue` fields in the definition that were created as
    /// `Unresolved(Result(idx))` placeholders are automatically resolved
    /// to point at this statement.
    #[builder(finish_fn = new)]
    pub fn statement(&mut self, #[builder(into)] definition: L) -> Statement {
        let id = self.statements.next_id();
        let owned_blocks: Vec<Block> = definition.blocks().copied().collect();
        let owned_cfgs: Vec<CFG> = definition.cfgs().copied().collect();
        let owned_digraphs: Vec<DiGraph> = definition.digraphs().copied().collect();
        let owned_ungraphs: Vec<UnGraph> = definition.ungraphs().copied().collect();

        for &block in &owned_blocks {
            let parent = self.blocks[block].parent;
            assert!(
                parent.is_none() || parent == Some(BlockParent::Statement(id)),
                "Block `{block}` already has a different parent"
            );
        }
        for &cfg in &owned_cfgs {
            let parent = self.cfgs[cfg].parent;
            assert!(
                parent.is_none() || parent == Some(id),
                "CFG `{cfg:?}` already has a different parent"
            );
        }
        for &graph in &owned_digraphs {
            let parent = self.digraphs[graph].parent;
            assert!(
                parent.is_none() || parent == Some(id),
                "DiGraph `{graph:?}` already has a different parent"
            );
        }
        for &graph in &owned_ungraphs {
            let parent = self.ungraphs[graph].parent;
            assert!(
                parent.is_none() || parent == Some(id),
                "UnGraph `{graph:?}` already has a different parent"
            );
        }

        let statement = StatementInfo {
            node: LinkedListNode::new(id),
            parent: None,
            definition,
        };
        let _ = self.statements.alloc(statement);

        for block in owned_blocks {
            self.blocks[block].parent = Some(BlockParent::Statement(id));
        }
        for cfg in owned_cfgs {
            self.cfgs[cfg].parent = Some(id);
        }
        for graph in owned_digraphs {
            self.digraphs[graph].parent = Some(id);
        }
        for graph in owned_ungraphs {
            self.ungraphs[graph].parent = Some(id);
        }

        // Resolve Unresolved(Result(idx)) SSAs now that the statement ID is known
        let result_ssas: Vec<SSAValue> = self.statements[id]
            .definition
            .results()
            .map(|rv| SSAValue::from(*rv))
            .collect();
        for ssa in result_ssas {
            if let Some(info) = self.ssas.get_mut(ssa)
                && let BuilderSSAKind::Unresolved(ResolutionInfo::Result(idx)) = info.kind
            {
                info.kind = BuilderSSAKind::Result(id, idx);
            }
        }

        id
    }

    /// Create a new staged function.
    ///
    /// `signature` is the interface the function exposes at this stage. The
    /// staged function owns it; each specialization added later declares its
    /// own signature on its definition statement and must have the same
    /// shape.
    #[builder(finish_fn = new)]
    pub fn staged_function(
        &mut self,
        name: Option<GlobalSymbol>,
        signature: Signature<L::Type>,
        backedges: Option<Vec<StagedFunction>>,
    ) -> Result<StagedFunction, StagedFunctionError<L>> {
        let sig = signature;

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
                .filter(|id| self.staged_functions[*id].signature() == &sig)
                .collect();

            if !duplicate_signature.is_empty() {
                return Err(StagedFunctionError {
                    conflict_kind: StagedFunctionConflictKind::DuplicateSignature,
                    name,
                    signature: sig,
                    conflicting: duplicate_signature,
                    backedges: backedges.unwrap_or_default(),
                });
            }

            if self.staged_name_policy == StagedNamePolicy::SingleInterface {
                let signature_mismatch: Vec<_> = same_name
                    .into_iter()
                    .filter(|id| self.staged_functions[*id].signature() != &sig)
                    .collect();

                if !signature_mismatch.is_empty() {
                    return Err(StagedFunctionError {
                        conflict_kind:
                            StagedFunctionConflictKind::SignatureMismatchUnderSingleInterface,
                        name,
                        signature: sig,
                        conflicting: signature_mismatch,
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
            specializations: Vec::new(),
            backedges: backedges.unwrap_or_default(),
            invalidated: false,
        };
        let _ = self.staged_functions.alloc(staged_function);
        Ok(id)
    }

    /// Add a specialization of `func` whose definition is `definition`.
    ///
    /// The definition statement owns the specialization's signature (see
    /// [`HasSignature`]); nothing is passed separately and nothing is stored
    /// on the specialization.
    ///
    /// # Errors
    ///
    /// - [`SpecializeError::UnknownDefinition`] if `definition` is not a
    ///   statement of this stage.
    /// - [`SpecializeError::MissingSignature`] if `definition` declares no
    ///   signature.
    /// - [`SpecializeError::InterfaceMismatch`] if its parameter or return
    ///   count differs from the staged function's interface.
    /// - [`SpecializeError::NotARefinement`] if the dialect's
    ///   [`Signatures`](Dialect::Signatures) says the signature does not
    ///   [refine](SignatureSemantics::refines) the interface.
    /// - [`SpecializeError::Duplicate`] if a live specialization already has
    ///   an equal signature.
    #[builder(finish_fn = new)]
    pub fn specialize(
        &mut self,
        #[builder(name = staged_func)] func: StagedFunction,
        #[builder(into)] definition: Statement,
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> Result<SpecializedFunction, SpecializeError>
    where
        L: HasSignature<L>,
    {
        let staged_function_info = &self.staged_functions[func];
        if staged_function_info.is_invalidated() {
            return Err(SpecializeError::InvalidatedStagedFunction(func));
        }

        let Some(definition_info) = self.statements.get(definition) else {
            return Err(SpecializeError::UnknownDefinition(definition));
        };
        let Some(signature) = definition_info.definition.signature() else {
            return Err(SpecializeError::MissingSignature(definition));
        };

        let interface = &staged_function_info.signature;
        if signature.params().len() != interface.params().len()
            || signature.rets().len() != interface.rets().len()
        {
            return Err(SpecializeError::InterfaceMismatch(InterfaceMismatch {
                staged_function: func,
                definition,
                params: (interface.params().len(), signature.params().len()),
                rets: (interface.rets().len(), signature.rets().len()),
            }));
        }

        if !L::Signatures::refines(signature, interface) {
            return Err(SpecializeError::NotARefinement {
                staged_function: func,
                definition,
            });
        }

        let conflicting: Vec<SpecializedFunction> = staged_function_info
            .specializations
            .iter()
            .filter(|s| !s.is_invalidated())
            .filter(|s| {
                self.statements
                    .get(*s.definition())
                    .and_then(|info| info.definition.signature())
                    == Some(signature)
            })
            .map(|s| s.id())
            .collect();

        if !conflicting.is_empty() {
            return Err(SpecializeError::Duplicate(DuplicateSpecialization {
                staged_function: func,
                conflicting,
                definition,
                backedges,
            }));
        }

        let staged_function_info = &mut self.staged_functions[func];
        let id = SpecializedFunction(func, staged_function_info.specializations.len());

        let specialized_function = SpecializedFunctionInfo::builder()
            .id(id)
            .definition(definition)
            .maybe_backedges(backedges)
            .new();
        staged_function_info
            .specializations
            .push(specialized_function);
        Ok(id)
    }
}
