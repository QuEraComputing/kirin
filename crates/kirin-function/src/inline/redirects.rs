use std::collections::HashMap;

use kirin::prelude::*;

/// The values that replace the results of the inlined calls, applied once
/// every call is inlined.
///
/// A replacement can be another inlined call's result: `%c2 = call @id(%c1)`
/// is replaced by `%c1`, which is itself replaced. Redirecting each call's
/// uses as it is inlined would move the same uses once per call along such a
/// chain. [`Redirects::apply`] instead sends each result straight to the value
/// its chain ends at, so each use is rewritten once.
#[derive(Default)]
pub(super) struct Redirects {
    /// What replaces each result, by result.
    to: HashMap<SSAValue, SSAValue>,
    /// The results in the order they were recorded, so the events of
    /// [`Redirects::apply`] do not depend on hashing.
    results: Vec<SSAValue>,
    /// The inlined calls, erased once nothing reads their results.
    calls: Vec<Statement>,
}

impl Redirects {
    /// Record that `call` was inlined and that `replacements[i]` replaces its
    /// `i`-th result.
    pub(super) fn record(
        &mut self,
        call: Statement,
        results: &[SSAValue],
        replacements: impl IntoIterator<Item = SSAValue>,
    ) {
        for (&result, replacement) in results.iter().zip(replacements) {
            self.to.insert(result, replacement);
            self.results.push(result);
        }
        self.calls.push(call);
    }

    /// The value `value`'s chain of replacements ends at: `value` itself if
    /// it is not a recorded result.
    ///
    /// Every result on the way is then pointed at that value, so a later
    /// lookup through it takes one step. The value a chain ends at is never a
    /// recorded result, so it is never replaced again, and each result is
    /// walked through at most once before it points there: resolving every
    /// result is linear in their number.
    fn resolve(&mut self, value: SSAValue) -> SSAValue {
        let mut end = value;
        while let Some(&next) = self.to.get(&end) {
            end = next;
        }
        let mut current = value;
        while current != end {
            let next = self.to.insert(current, end).expect("on the chain");
            current = next;
        }
        end
    }

    /// Redirect every use of each recorded result to the value its chain ends
    /// at, then erase the inlined calls, which nothing reads any more.
    ///
    /// No result is redirected to a recorded result, so redirecting one adds
    /// no uses to another still waiting: each use is rewritten once.
    pub(super) fn apply<L: Dialect>(
        mut self,
        rewriter: &mut Rewriter<'_, L>,
    ) -> Result<(), RewriteError> {
        for result in std::mem::take(&mut self.results) {
            let end = self.resolve(result);
            rewriter.replace_all_uses(result, end)?;
        }
        for call in self.calls {
            rewriter.erase_statement(call)?;
        }
        Ok(())
    }
}
