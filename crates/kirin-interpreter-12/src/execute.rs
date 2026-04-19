use crate::control::Control;
use crate::env::Env;

/// A cursor paired with a pending inbox value from a completed child body.
///
/// The inbox delivers the yield/return value from a child body (e.g. an
/// `scf.if` or `scf.for` body) to its parent cursor. On the next call to
/// `Execute::execute`, the cursor reads this inbox instead of requiring a
/// separate communication channel.
pub struct StackEntry<C, V> {
    pub cursor: C,
    pub inbox: Option<V>,
}

impl<C, V> StackEntry<C, V> {
    pub fn new(cursor: C) -> Self {
        Self {
            cursor,
            inbox: None,
        }
    }

    pub fn with_inbox(cursor: C, inbox: V) -> Self {
        Self {
            cursor,
            inbox: Some(inbox),
        }
    }
}

/// Cursor execution trait.
///
/// `inbox` carries the yield value delivered from the last child body that
/// completed. Cursors like `IfCursor` and `ForCursor` await a `Some(v)` inbox
/// before writing their results and popping.
///
/// `BlockCursor` ignores `inbox`: the outer SCF cursor, not the block cursor,
/// receives the yield from the body it initiated.
pub trait Execute<E: Env> {
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<E::Value>,
    ) -> Result<Control<E::Value, E::Ext>, E::Error>;
}
