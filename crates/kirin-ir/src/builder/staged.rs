use super::error::{SpecializeError, StagedFunctionConflictKind, StagedFunctionError};
use super::stage_info::BuilderStageInfo;

use crate::Dialect;
use crate::node::symbol::GlobalSymbol;
use crate::node::*;
use crate::signature::Signature;

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
        self.stage.ssas.alloc(ssa);
        id
    }

    /// Look up by name (matched against the builder's `.port_name()` / `.arg_name()` / `.capture_name()` declarations).
    pub fn name(self, name: &str) -> SSAValue {
        let symbol = self.stage.symbols.intern(name.to_string());
        let kind = self.make_ssa_kind(BuilderKey::Named(symbol));
        let id = self.stage.ssas.next_id();
        let ssa = BuilderSSAInfo::new(id, None, None, kind);
        self.stage.ssas.alloc(ssa);
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

    /// Create a statement from a dialect definition.
    ///
    /// Any `ResultValue` fields in the definition that were created as
    /// `Unresolved(Result(idx))` placeholders are automatically resolved
    /// to point at this statement.
    #[builder(finish_fn = new)]
    pub fn statement(&mut self, #[builder(into)] definition: L) -> Statement {
        let id = self.statements.next_id();
        let statement = StatementInfo {
            node: LinkedListNode::new(id),
            parent: None,
            definition,
        };
        self.statements.alloc(statement);

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
        let sig = signature.unwrap_or_else(Signature::placeholder);

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
                    specializations: specializations.unwrap_or_default(),
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

    #[builder(finish_fn = new)]
    pub fn specialize(
        &mut self,
        #[builder(name = staged_func)] func: StagedFunction,
        signature: Option<Signature<L::Type>>,
        #[builder(into)] body: Statement,
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> Result<SpecializedFunction, SpecializeError<L>> {
        let staged_function_info = &mut self.staged_functions[func];

        let signature = signature.unwrap_or(staged_function_info.signature.clone());

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
