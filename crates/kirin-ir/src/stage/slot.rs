use crate::node::symbol::GlobalSymbol;
use crate::{Quarantined, StageMeta};

/// One stage position in a [`Pipeline`](crate::Pipeline).
///
/// A pass takes exclusive ownership of the stage it rewrites, so a slot must be
/// able to be empty. It must *not* be emptied with `mem::take`: a default
/// [`StageInfo`](crate::StageInfo) is valid, well-formed, empty IR, so a failed
/// pass would silently become "this stage compiled to nothing".
///
/// A position must stay identifiable by name in every state, but two of the
/// three have no readable stage: a transferred slot holds nothing, and a
/// poisoned one holds its stage inside a [`Quarantined`], which lends out no
/// IR. So the stage's own name is the only copy while it is present, and the
/// slot records it at the moment the stage is transferred. Nothing can rename
/// a stage that is out of reach, so the recorded copy cannot drift from it.
///
/// [`StageSlot`] is a separate type so the state enum [`SlotState`] remains
/// private to this module. This way, it changes only through the transitions
/// below, which refuse to overwrite a present stage, revive a poisoned one, or
/// record a name the stage did not carry.
pub(crate) struct StageSlot<S>(SlotState<S>);

/// What a [`StageSlot`] currently holds.
enum SlotState<S> {
    /// Holds a stage, the ordinary state.
    Present(S),
    /// The stage is currently lent to a running pass.
    ///
    /// This state is reachable only if something unwinds between taking
    /// the stage out and putting it back, outside the `catch_unwind` that
    /// [`Pipeline::run_pass`](crate::rewrite::pass::drive_pass) installs
    /// Hence, it marks a stage lost to a panic, rather than one never written back.
    Transferred { name: Option<GlobalSymbol> },
    /// A pass failed here. The slot keeps its position so every previously
    /// issued [`CompileStage`](crate::CompileStage), which is a raw index,
    /// stays valid.
    Poisoned {
        name: Option<GlobalSymbol>,
        quarantined: Quarantined<S>,
    },
}

/// Which of three states a stage position in a [`Pipeline`](crate::Pipeline)
/// is in, named without borrowing what it holds.
///
/// Read with [`Pipeline::stage_status`](crate::Pipeline::stage_status).
/// [`Pipeline::stage`](crate::Pipeline::stage) collapses the latter two into
/// `None`, which is the right answer for "can I use this?" but not for "why
/// not?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageStatus {
    /// The slot holds a stage.
    Present,
    /// The stage is lent to a pass that has not given it back.
    Transferred,
    /// A pass failed here; the slot holds a [`Quarantined`] in place of a stage.
    Poisoned,
}

impl<S> StageSlot<S> {
    /// A slot holding `stage`, answering to whatever name the stage carries.
    pub(crate) fn present(stage: S) -> Self {
        Self(SlotState::Present(stage))
    }

    /// Lend the stage out, leaving this position marked as transferred.
    ///
    /// Returns `None` when the slot holds no stage, leaving it exactly as it was.
    #[allow(dead_code)]
    pub(crate) fn take(&mut self) -> Option<S>
    where
        S: StageMeta,
    {
        // Read the name while the stage is still here: once it leaves, the
        // marker is the only thing left to answer to it.
        let name = self.stage()?.stage_name();
        // Swapping in the marker and reading the old state is one step, so the
        // slot is never left holding neither.
        match std::mem::replace(&mut self.0, SlotState::Transferred { name }) {
            SlotState::Present(stage) => Some(stage),
            SlotState::Transferred { .. } | SlotState::Poisoned { .. } => {
                unreachable!("`stage()` just found this slot present")
            }
        }
    }

    /// Restore a `StageSlot` that was lent out
    ///
    /// Panics when the `SlotState` is not `Transferred` since we should not be
    /// overwriting a `Present` or `Poisoned` slot
    #[allow(dead_code)]
    pub(crate) fn restore(&mut self, stage: S) {
        match &self.0 {
            SlotState::Transferred { .. } => {
                self.0 = SlotState::Present(stage);
            }
            _ => panic!("Can only restore a Transferred StageSlot"),
        }
    }

    /// Poison a `StageSlot` that was lent out after the stage fails in a pass
    ///
    /// Panics when the `SlotState` is not `Transferred` since we should not be
    /// overwriting a `Present` or `Poisoned` slot
    #[allow(dead_code)]
    pub(crate) fn poison(&mut self, quarantined: Quarantined<S>) {
        match self.0 {
            SlotState::Transferred { name } => {
                self.0 = SlotState::Poisoned { name, quarantined };
            }
            _ => panic!("Can only poison a Transferred StageSlot"),
        }
    }

    /// The artifact left by the pass that failed at this position, if any.
    ///
    /// `None` unless the slot is poisoned. This is the only way to reach the
    /// cause, the mutation events, and the rendered report: [`Quarantined`]
    /// deliberately lends out no IR, so nothing else survives a failed pass.
    #[allow(dead_code)]
    pub(crate) fn quarantined(&self) -> Option<&Quarantined<S>> {
        match &self.0 {
            SlotState::Poisoned { quarantined, .. } => Some(quarantined),
            _ => None,
        }
    }

    /// Which state this slot is in, without borrowing the stage or the
    /// [`Quarantined`] it may hold.
    #[allow(dead_code)]
    pub(crate) fn status(&self) -> StageStatus {
        match &self.0 {
            SlotState::Present(_) => StageStatus::Present,
            SlotState::Transferred { .. } => StageStatus::Transferred,
            SlotState::Poisoned { .. } => StageStatus::Poisoned,
        }
    }

    /// The interned name this position answers to, in every state.
    ///
    /// The stage's own name while it is present, so renaming the stage renames
    /// the position; the name it left with otherwise.
    pub(crate) fn name(&self) -> Option<GlobalSymbol>
    where
        S: StageMeta,
    {
        match &self.0 {
            SlotState::Present(stage) => stage.stage_name(),
            SlotState::Transferred { name } | SlotState::Poisoned { name, .. } => *name,
        }
    }

    /// The stage, if this slot still holds one.
    pub(crate) fn stage(&self) -> Option<&S> {
        match &self.0 {
            SlotState::Present(stage) => Some(stage),
            SlotState::Transferred { .. } | SlotState::Poisoned { .. } => None,
        }
    }

    /// The stage, mutably, if this slot still holds one.
    pub(crate) fn stage_mut(&mut self) -> Option<&mut S> {
        match &mut self.0 {
            SlotState::Present(stage) => Some(stage),
            SlotState::Transferred { .. } | SlotState::Poisoned { .. } => None,
        }
    }
}

/// Serializes as the stage it holds, so a pipeline's stages stay a plain list
/// on the wire and its name travels with the stage.
///
/// A position without a stage is an error rather than skipped: every serialized
/// [`CompileStage`](crate::CompileStage) is a raw index into that list, so
/// dropping one would point every later id at the wrong stage.
#[cfg(feature = "serde")]
impl<S: serde::Serialize> serde::Serialize for StageSlot<S> {
    fn serialize<Ser: serde::Serializer>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error> {
        match self.stage() {
            Some(stage) => stage.serialize(serializer),
            None => Err(serde::ser::Error::custom(format_args!(
                "cannot serialize a pipeline position whose stage is unavailable: {:?}",
                self.status()
            ))),
        }
    }
}
