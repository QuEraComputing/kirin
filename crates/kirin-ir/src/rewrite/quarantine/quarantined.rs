//! The failure artifact of a rewrite pass.
//!
//! A pass that fails part-way through leaves edits already applied. The stage
//! is not rolled back. Instead, [`Quarantined`] takes ownership of it so that it can
//! still be inspected but can no longer be mistaken for usable IR.

use std::fmt;

use crate::{Dialect, MutationEvent, StageInfo};

use super::cause::QuarantineCause;
use super::report;

/// Whatever a failed pass left in an unknown state.
///
/// `S` is what the quarantine took custody of, typically a [`StageInfo`].
/// A pass over a standalone stage yields `Quarantined<StageInfo<L>>`;
/// a pass over a stage that lives in a [`Pipeline`](crate::Pipeline) yields
/// `Quarantined<S>` over the whole stage container, which is what lets a
/// container wrapping several dialects be quarantined without naming them.
///
/// Access to the poisoned stage is restricted; the caller can never regain ordinary
/// access to the stage, forcing them to abandon this compilation unit. The
/// diagnostic report is rendered once, at construction, while the dialect is
/// still known — see [`report`].
///
/// Three features are missing by design and must not be "fixed":
///
/// - **no `Clone`** — a quarantined stage is evidence, and a copy of it invites
///   being passed around until something treats it as usable.
/// - **no `Default`** — otherwise a container holding a stage can be emptied
///   with `mem::take`, and a default [`StageInfo`] is *valid, well-formed,
///   empty* IR, so the failure would silently become "this stage compiled to
///   nothing".
/// - **no accessor returning the poisoned stage** — we want to prevent a caller
///   from using the `&StageInfo` object, with which every ordinary query works
///   again and the quarantine means nothing.
pub struct Quarantined<S> {
    /// Held, never lent out: the poisoned stage is what makes "you cannot use this
    /// stage again" physical rather than advisory.
    ///
    /// Never read by design — the report is rendered from it at construction,
    /// and there is deliberately no accessor.
    _poisoned_stage: S,
    cause: QuarantineCause,
    events: Vec<MutationEvent>,
    report: String,
}

impl<S> Quarantined<S> {
    /// Take custody of `payload` with an already-rendered `report`.
    ///
    /// Crate-private because rendering and construction must not drift apart:
    /// a caller that forgot to render would produce an artifact with an empty
    /// report, and the failure would only surface when someone needed it.
    /// Callers that hold a stage should use [`Quarantined::from_stage`].
    pub(crate) fn new(
        _poisoned_stage: S,
        cause: QuarantineCause,
        events: Vec<MutationEvent>,
        report: String,
    ) -> Self {
        Self {
            _poisoned_stage,
            cause,
            events,
            report,
        }
    }

    pub fn cause(&self) -> &QuarantineCause {
        &self.cause
    }

    pub fn events(&self) -> &[MutationEvent] {
        &self.events
    }

    /// The diagnostic report, rendered when this artifact was built.
    pub fn report(&self) -> &str {
        &self.report
    }
}

impl<L: Dialect> Quarantined<StageInfo<L>> {
    /// Take ownership of a stage a failed pass has already mutated, rendering
    /// its report before the stage moves in.
    ///
    /// [`run_pass`](crate::run_pass) is the only caller in ordinary use; this
    /// stays public so that tests can build a `Quarantined` over a stage
    /// corrupted on purpose, which no pass can produce through a
    /// [`Rewriter`](crate::Rewriter).
    pub fn from_stage(
        stage: StageInfo<L>,
        cause: QuarantineCause,
        events: Vec<MutationEvent>,
    ) -> Self {
        let report = report::render(&stage, &events);
        Self::new(stage, cause, events, report)
    }
}

/// Manual so that `{:?}` stays readable: a derived impl would inline the
/// payload — every arena of a stage — and the whole rendered report on every
/// debug print, drowning test output. It would also force `S: Debug`. Use
/// [`Quarantined::report`] for the contents.
impl<S> fmt::Debug for Quarantined<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Quarantined")
            .field("cause", &self.cause)
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl<S> fmt::Display for Quarantined<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stage quarantined after {} mutation event(s): {}",
            self.events.len(),
            self.cause,
        )
    }
}

impl<S> std::error::Error for Quarantined<S> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.cause {
            QuarantineCause::Pass(error) => Some(error.as_ref()),
            QuarantineCause::Verify(error) => Some(error),
            QuarantineCause::Panic(_) => None,
        }
    }
}
