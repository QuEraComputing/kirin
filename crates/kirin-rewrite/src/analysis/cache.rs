use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use super::AnalysisKind;

/// One cached entry of an analysis's table.
pub(crate) enum Slot<T> {
    /// Being computed. Finding this while looking the entry up means the
    /// computation asked for itself.
    InProgress,
    Done {
        value: Arc<T>,
        /// The revision this result was computed at.
        verified_at: u64,
    },
}

/// The results of one analysis, by key.
pub(crate) type Table<A> = HashMap<<A as AnalysisKind>::Key, Slot<<A as AnalysisKind>::Output>>;

/// Cached analysis results, read-only to everyone outside the database. A
/// pass and its rules read them.
#[derive(Default)]
pub struct AnalysisCache {
    /// One [`Table`] per analysis, by the analysis's `TypeId`.
    tables: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    /// The program's revision: bumped once for each pass that edits it.
    revision: u64,
}

impl AnalysisCache {
    /// The cached result of `A` for `key`, never computing.
    pub fn get<A: AnalysisKind>(&self, key: &A::Key) -> Option<Arc<A::Output>> {
        let table = self.tables.get(&TypeId::of::<A>())?;
        match downcast::<A>(table.as_ref()).get(key)? {
            Slot::Done { value, verified_at } if *verified_at == self.revision => {
                Some(Arc::clone(value))
            }
            _ => None,
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// `A`'s table, created empty if `A` has none yet.
    pub(crate) fn table_mut<A: AnalysisKind>(&mut self) -> &mut Table<A> {
        self.tables
            .entry(TypeId::of::<A>())
            .or_insert_with(|| Box::new(Table::<A>::new()))
            .downcast_mut()
            .expect("a table is stored under its analysis's TypeId")
    }

    /// Move to the next revision and free every result, all now stale.
    pub(crate) fn bump(&mut self) {
        self.revision += 1;
        self.tables.clear();
    }
}

fn downcast<A: AnalysisKind>(table: &(dyn Any + Send + Sync)) -> &Table<A> {
    table
        .downcast_ref()
        .expect("a table is stored under its analysis's TypeId")
}
