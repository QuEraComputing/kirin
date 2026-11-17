use std::any::{Any, TypeId, type_name};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use kirin_ir::{
    CompileStage, Dialect, HasStageInfo, Program, Rewriter, StageMeta, StagePassError,
};

use super::cache::Slot;
use super::{Analysis, AnalysisCache, AnalysisError, AnalysisKind, Requirement, RunPassError};

/// Computes a requirement's analysis for its type-erased key.
type Compute<S> = fn(&Database<S>, &(dyn Any + Send + Sync)) -> Result<(), AnalysisError>;

/// Owns a program and the analysis results computed from it, as a salsa
/// database owns its inputs and memoized queries.
///
/// Every edit to the program goes through the database (a pass through
/// [`run_pass`](Self::run_pass), anything else through
/// [`program_mut`](Self::program_mut)), so no cached result outlives an
/// edit: each edit bumps the revision and frees every cached result.
pub struct Database<S> {
    program: Program<S>,
    /// How to compute each analysis a pass may require, by its `TypeId`.
    registered: HashMap<TypeId, Compute<S>>,
    /// In a `RefCell`, which allows mutation through a shared reference
    /// (checked at run time), so that [`get`](Self::get) can fill the cache
    /// through `&self`.
    cache: RefCell<AnalysisCache>,
}

impl<S> Database<S> {
    /// A database over `program`, with nothing cached or registered.
    pub fn new(program: Program<S>) -> Self {
        Self {
            program,
            registered: HashMap::new(),
            cache: RefCell::default(),
        }
    }

    /// Let passes require `A`. [`get`](Self::get) needs no registration.
    pub fn register<A: Analysis<S>>(&mut self) -> &mut Self {
        self.registered.insert(TypeId::of::<A>(), compute::<S, A>);
        self
    }

    /// The cached result of `A` for `key`, or compute it (and what it
    /// depends on) and cache it. A failure is returned, not cached: the next
    /// `get` runs the analysis again.
    ///
    /// The cache is not borrowed while `A::run` runs, so `run` can call `get`
    /// itself. Asking for `A` for the same `key` from inside that `run`
    /// fails with [`AnalysisError::Cycle`].
    pub fn get<A: Analysis<S>>(&self, key: &A::Key) -> Result<Arc<A::Output>, AnalysisError> {
        {
            let mut cache = self.cache.borrow_mut();
            let revision = cache.revision();
            let table = cache.table_mut::<A>();
            match table.get(key) {
                Some(Slot::Done { value, verified_at }) if *verified_at == revision => {
                    return Ok(Arc::clone(value));
                }
                Some(Slot::InProgress) => {
                    return Err(AnalysisError::Cycle {
                        analysis: type_name::<A>(),
                    });
                }
                _ => {
                    table.insert(key.clone(), Slot::InProgress);
                }
            }
        }

        // Clears the in-progress mark unless the result replaces it, so a
        // failed or panicking `run` leaves nothing behind.
        let _mark = InProgress::<A> {
            cache: &self.cache,
            key,
        };
        let value = Arc::new(A::run(self, key)?);
        let mut cache = self.cache.borrow_mut();
        let verified_at = cache.revision();
        if let Some(slot) = cache.table_mut::<A>().get_mut(key) {
            *slot = Slot::Done {
                value: Arc::clone(&value),
                verified_at,
            };
        }
        Ok(value)
    }

    /// The cached result of `A` for `key`, never computing.
    pub fn get_cached<A: AnalysisKind>(&self, key: &A::Key) -> Option<Arc<A::Output>> {
        self.cache.borrow().get::<A>(key)
    }

    pub fn program(&self) -> &Program<S> {
        &self.program
    }

    /// For edits that are not passes, such as parsing more functions in.
    /// Bumps the revision, since the database cannot see what changes.
    pub fn program_mut(&mut self) -> &mut Program<S> {
        self.cache.get_mut().bump();
        &mut self.program
    }

    /// The program, discarding the cached results.
    pub fn into_program(self) -> Program<S> {
        self.program
    }

    /// How many passes have edited the program, plus how many times
    /// [`program_mut`](Self::program_mut) was called, since the database
    /// was created.
    pub fn revision(&self) -> u64 {
        self.cache.borrow().revision()
    }

    /// Compute every requirement in `requires` that is not cached, then run
    /// `pass` on `stage` through [`Program::run_pass`], handing it the
    /// cached results. Bump the revision if the pass edited the IR, or
    /// failed, since a failed pass may have left some of its edits in place.
    ///
    /// A requirement that cannot be computed, including one never
    /// [registered](Self::register), fails before the pass runs, so the stage
    /// is untouched.
    pub fn run_pass<L, T, E>(
        &mut self,
        stage: CompileStage,
        requires: impl IntoIterator<Item = Requirement>,
        pass: impl FnOnce(&mut Rewriter<L>, &AnalysisCache) -> Result<T, E>,
    ) -> Result<T, RunPassError>
    where
        L: Dialect,
        S: StageMeta + HasStageInfo<L>,
        E: Error + Send + Sync + 'static,
    {
        for requirement in requires {
            let Some(&compute) = self.registered.get(&requirement.analysis()) else {
                return Err(RunPassError::RequirementUnsatisfied(
                    AnalysisError::Unregistered {
                        analysis: requirement.name(),
                    },
                ));
            };
            compute(self, requirement.key()).map_err(RunPassError::RequirementUnsatisfied)?;
        }

        let cache = self.cache.get_mut();
        let mut edited = false;
        let result = self.program.run_pass(stage, |rewriter| {
            let output = pass(rewriter, cache);
            edited = rewriter.edited();
            output
        });
        if edited || matches!(result, Err(StagePassError::PassFailed)) {
            self.cache.get_mut().bump();
        }
        result.map_err(RunPassError::PassFailedOrNotRun)
    }
}

/// The registry's entry for `A`: compute it for a requirement's key.
fn compute<S, A: Analysis<S>>(
    db: &Database<S>,
    key: &(dyn Any + Send + Sync),
) -> Result<(), AnalysisError> {
    let key = key
        .downcast_ref::<A::Key>()
        .expect("a requirement's key has its analysis's key type");
    db.get::<A>(key).map(drop)
}

/// The in-progress mark on `A`'s entry for `key`, cleared on drop unless a
/// result replaced it.
struct InProgress<'a, A: AnalysisKind> {
    cache: &'a RefCell<AnalysisCache>,
    key: &'a A::Key,
}

impl<A: AnalysisKind> Drop for InProgress<'_, A> {
    fn drop(&mut self) {
        // If the cache is already borrowed, leave the mark rather than panic.
        let Ok(mut cache) = self.cache.try_borrow_mut() else {
            return;
        };
        let table = cache.table_mut::<A>();
        // Still in progress means `run` failed or panicked: clear the mark so
        // the failure is not cached and no false cycle remains. On success
        // `get` has already replaced it with the result, which must stay.
        if matches!(table.get(self.key), Some(Slot::InProgress)) {
            table.remove(self.key);
        }
    }
}
