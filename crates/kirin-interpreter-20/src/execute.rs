use crate::control::Control;
use crate::env::Env;

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

pub trait Execute<E: Env> {
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<E::Value>,
    ) -> Result<Control<E::Value, E::Ext>, E::Error>;
}
