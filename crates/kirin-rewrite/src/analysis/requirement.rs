use std::any::{Any, TypeId, type_name};
use std::sync::Arc;

use super::AnalysisKind;

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
}

impl Requirement {
    /// Analysis `A`, for `key`.
    pub fn of<A: AnalysisKind>(key: A::Key) -> Self {
        Self {
            analysis: TypeId::of::<A>(),
            name: type_name::<A>(),
            key: Arc::new(key),
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
}
