mod fixpoint;
mod interp;
mod stage;
mod summary;

pub use interp::{AbstractInterpreter, SummaryInserter};
pub use summary::{SummaryCache, SummaryEntry};

use kirin_ir::{Block, SSAValue};
use rustc_hash::FxHashMap;

use crate::DedupScheduler;

/// Per-function fixpoint state stored as frame extra data.
///
/// Block argument SSA value IDs are tracked here; the actual SSA values
/// (both block args and statement results) live in [`Frame::values`].
#[derive(Debug, Default)]
pub struct FixpointState {
    pub(crate) worklist: DedupScheduler<Block>,
    /// Per-block argument SSA value IDs. Key presence = block visited.
    pub(crate) block_args: FxHashMap<Block, Vec<SSAValue>>,
    /// Per-block visit counts for [`WideningStrategy::Delayed`].
    pub(crate) visit_counts: FxHashMap<Block, usize>,
}
