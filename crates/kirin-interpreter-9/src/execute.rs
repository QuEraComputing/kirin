use crate::control::Control;
use crate::env::Env;

/// A cursor with its pending inbox value.
///
/// The inbox holds the yield/return value delivered from a child body
/// (e.g. an scf.if or scf.for body). On the next call to `Execute::execute`,
/// the cursor receives this inbox instead of consulting a `pending_yield` field.
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
/// completed. Cursors like `IfCursor` and `ForCursor` wait for a `Some(v)`
/// inbox before writing their results and popping.
///
/// `BlockCursor` ignores `inbox` (block bodies do not receive yield values
/// directly; their outer SCF cursor does).
pub trait Execute<E: Env> {
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<E::Value>,
    ) -> Result<Control<E::Value, E::Ext>, E::Error>;
}
