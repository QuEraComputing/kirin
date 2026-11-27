//! A per-thread record of every analysis-client run: which analysis, how long
//! each root took, and the total. Tests count runs with it; the benchmark
//! reads the times.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use kirin::prelude::SpecializedFunction;

/// One run of a stage-wide analysis client.
#[derive(Clone, Debug)]
pub struct Run {
    pub analysis: &'static str,
    /// Each root's time, in the order they ran.
    pub roots: Vec<(SpecializedFunction, Duration)>,
    pub total: Duration,
    started: Instant,
}

/// A root being timed; hand it back to [`Run::end_root`].
pub struct Root(SpecializedFunction, Instant);

impl Run {
    pub fn start(analysis: &'static str) -> Self {
        Self {
            analysis,
            roots: Vec::new(),
            total: Duration::ZERO,
            started: Instant::now(),
        }
    }

    pub fn root(&self, function: SpecializedFunction) -> Root {
        Root(function, Instant::now())
    }

    pub fn end_root(&mut self, Root(function, started): Root) {
        self.roots.push((function, started.elapsed()));
    }

    /// Record the run on this thread's log.
    pub fn finish(mut self) {
        self.total = self.started.elapsed();
        LOG.with_borrow_mut(|log| log.push(self));
    }
}

thread_local! {
    static LOG: RefCell<Vec<Run>> = const { RefCell::new(Vec::new()) };
    static LOOKUPS: RefCell<Lookups> = const { RefCell::new(Lookups { count: 0, time: Duration::ZERO }) };
}

/// Fact reads by the rules: how many, and their total time.
#[derive(Clone, Copy, Debug, Default)]
pub struct Lookups {
    pub count: usize,
    pub time: Duration,
}

/// Run `read`, a rule's read of an analysis result, adding it to the lookups.
pub fn lookup<R>(read: impl FnOnce() -> R) -> R {
    let started = Instant::now();
    let value = read();
    LOOKUPS.with_borrow_mut(|lookups| {
        lookups.count += 1;
        lookups.time += started.elapsed();
    });
    value
}

/// The lookups since the last call.
pub fn take_lookups() -> Lookups {
    LOOKUPS.with_borrow_mut(std::mem::take)
}

/// Every run recorded on this thread since the last `take`, oldest first.
pub fn take() -> Vec<Run> {
    LOG.with_borrow_mut(std::mem::take)
}
