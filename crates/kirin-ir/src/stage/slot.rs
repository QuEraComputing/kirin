use crate::Quarantined;
use crate::node::symbol::GlobalSymbol;

/// One stage position in a [`Pipeline`](crate::Pipeline).
///
/// A pass takes exclusive ownership of the stage it rewrites, so a slot must be
/// able to be empty. It must *not* be emptied with `mem::take`: a default
/// [`StageInfo`](crate::StageInfo) is valid, well-formed, empty IR, so a failed
/// pass would silently become "this stage compiled to nothing".
///
/// The name lives on the slot rather than only on the stage because a position
/// must stay identifiable in every state, and two of the three have no readable
/// stage: a transferred slot holds nothing, and a poisoned one holds its stage
/// inside a [`Quarantined`], which lends out no IR.
pub(crate) struct StageSlot<S> {
    name: Option<GlobalSymbol>,
    state: SlotState<S>,
}

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
    Transferred,
    /// A pass failed here. The slot keeps its position so every previously
    /// issued [`CompileStage`](crate::CompileStage), which is a raw index,
    /// stays valid.
    Poisoned(Quarantined<S>),
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
    /// A slot holding `stage`, optionally under `name`.
    pub(crate) fn present(stage: S, name: Option<GlobalSymbol>) -> Self {
        Self {
            name,
            state: SlotState::Present(stage),
        }
    }

    /// Lend the stage out, leaving this position marked as transferred.
    ///
    /// Returns `None` when the slot holds no stage, leaving it exactly as it was.
    #[allow(dead_code)]
    pub(crate) fn take(&mut self) -> Option<S> {
        // Swapping in the marker and reading the old state is one step, so the
        // slot is never left holding neither.
        match std::mem::replace(&mut self.state, SlotState::Transferred) {
            SlotState::Present(stage) => Some(stage),
            unchanged => {
                self.state = unchanged;
                None
            }
        }
    }

    /// Restore a `StageSlot` that was lent out
    ///
    /// Panics when the `SlotState` is not `Transferred` since we should not be
    /// overwriting a `Present` or `Poisoned` slot
    #[allow(dead_code)]
    pub(crate) fn restore(&mut self, stage: S) {
        match &self.state {
            SlotState::Transferred => {
                self.state = SlotState::Present(stage);
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
        match &self.state {
            SlotState::Transferred => {
                self.state = SlotState::Poisoned(quarantined);
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
        match &self.state {
            SlotState::Poisoned(q) => Some(q),
            _ => None,
        }
    }

    /// Which state this slot is in, without borrowing the stage or the
    /// [`Quarantined`] it may hold.
    #[allow(dead_code)]
    pub(crate) fn status(&self) -> StageStatus {
        match &self.state {
            SlotState::Present(_) => StageStatus::Present,
            SlotState::Transferred => StageStatus::Transferred,
            SlotState::Poisoned(_) => StageStatus::Poisoned,
        }
    }

    /// The interned name this position answers to, in every state.
    pub(crate) fn name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    /// The stage, if this slot still holds one.
    pub(crate) fn stage(&self) -> Option<&S> {
        match &self.state {
            SlotState::Present(stage) => Some(stage),
            SlotState::Transferred | SlotState::Poisoned(_) => None,
        }
    }

    /// The stage, mutably, if this slot still holds one.
    pub(crate) fn stage_mut(&mut self) -> Option<&mut S> {
        match &mut self.state {
            SlotState::Present(stage) => Some(stage),
            SlotState::Transferred | SlotState::Poisoned(_) => None,
        }
    }
}
