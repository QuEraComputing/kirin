use std::any::{Any, TypeId, type_name};
use std::sync::Arc;

use super::{AnalysisCache, AnalysisKind};

/// "This pass reads analysis `A` for `key`."
///
/// Unlike `A: Analysis<S>`, it does not mention the program's stage type, so
/// a `RuleSet<L>`, which only knows its language, can hold one. The key is
/// stored without its type; the database's registry turns it back into
/// `A::Key` when the requirement is computed.
#[derive(Clone, Debug)]
pub struct Requirement {
    analysis: TypeId,
    name: &'static str,
    key: Arc<dyn Any + Send + Sync>,
    /// Whether a cache holds `A`'s result for the key. Set where `A` is
    /// known, since the key's type is erased here.
    is_cached: fn(&AnalysisCache, &(dyn Any + Send + Sync)) -> bool,
}

impl Requirement {
    /// Analysis `A`, for `key`.
    pub fn of<A: AnalysisKind>(key: A::Key) -> Self {
        Self {
            analysis: TypeId::of::<A>(),
            name: type_name::<A>(),
            key: Arc::new(key),
            is_cached: is_cached::<A>,
        }
    }

    /// The analysis's type name, for errors.
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn analysis(&self) -> TypeId {
        self.analysis
    }

    pub(crate) fn key(&self) -> &(dyn Any + Send + Sync) {
        &*self.key
    }

    /// Whether `cache` holds this requirement's result.
    pub(crate) fn is_cached_in(&self, cache: &AnalysisCache) -> bool {
        (self.is_cached)(cache, self.key())
    }
}

fn is_cached<A: AnalysisKind>(cache: &AnalysisCache, key: &(dyn Any + Send + Sync)) -> bool {
    let key = key
        .downcast_ref::<A::Key>()
        .expect("a requirement's key has its analysis's key type");
    cache.get::<A>(key).is_some()
}
