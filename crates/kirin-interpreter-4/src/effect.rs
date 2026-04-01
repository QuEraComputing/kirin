use kirin_ir::{Block, ResultValue, SpecializedFunction};

/// An effect that means "advance to next statement".
pub trait IsAdvance {
    fn is_advance(&self) -> bool;
}

/// An effect that means "jump to a block with arguments".
pub trait IsJump {
    type Value;
    fn as_jump(&self) -> Option<(Block, &[Self::Value])>;
}

/// An effect that means "call a function".
pub trait IsCall {
    type Value;
    fn as_call(&self) -> Option<CallEffect<'_, Self::Value>>;
}

/// An effect that means "return from current function".
pub trait IsReturn {
    type Value;
    fn as_return(&self) -> Option<&Self::Value>;
}

/// An effect that means "yield from current inline execution".
pub trait IsYield {
    type Value;
    fn as_yield(&self) -> Option<&Self::Value>;
}

/// An effect that means "push a new cursor entry onto the stack".
pub trait IsPush {
    type CursorEntry;
    #[allow(clippy::wrong_self_convention)]
    fn as_push(self) -> Option<Self::CursorEntry>;
}

/// Borrowed view of a call effect's data.
pub struct CallEffect<'a, V> {
    pub callee: SpecializedFunction,
    pub args: &'a [V],
    pub results: &'a [ResultValue],
}

/// Convenience base effect for cursor-related control flow.
#[derive(Debug, Clone)]
pub enum CursorEffect<V> {
    Advance,
    Jump(Block, Vec<V>),
}

impl<V> IsAdvance for CursorEffect<V> {
    fn is_advance(&self) -> bool {
        matches!(self, CursorEffect::Advance)
    }
}

impl<V> IsJump for CursorEffect<V> {
    type Value = V;

    fn as_jump(&self) -> Option<(Block, &[Self::Value])> {
        match self {
            CursorEffect::Jump(block, args) => Some((*block, args)),
            _ => None,
        }
    }
}

// Unit effect: no effect produced.

impl IsAdvance for () {
    fn is_advance(&self) -> bool {
        false
    }
}

impl IsJump for () {
    type Value = ();

    fn as_jump(&self) -> Option<(Block, &[Self::Value])> {
        None
    }
}

impl IsCall for () {
    type Value = ();

    fn as_call(&self) -> Option<CallEffect<'_, Self::Value>> {
        None
    }
}

impl IsReturn for () {
    type Value = ();

    fn as_return(&self) -> Option<&Self::Value> {
        None
    }
}

impl IsYield for () {
    type Value = ();

    fn as_yield(&self) -> Option<&Self::Value> {
        None
    }
}

impl IsPush for () {
    type CursorEntry = ();

    fn as_push(self) -> Option<Self::CursorEntry> {
        None
    }
}

// ---------------------------------------------------------------------------
// Marker trait impls for Action<V, R, C>
// ---------------------------------------------------------------------------

use crate::concrete::Action;

impl<V, R, C> IsAdvance for Action<V, R, C> {
    fn is_advance(&self) -> bool {
        matches!(self, Action::Advance)
    }
}

impl<V, R, C> IsJump for Action<V, R, C> {
    type Value = V;

    fn as_jump(&self) -> Option<(Block, &[Self::Value])> {
        match self {
            Action::Jump(block, args) => Some((*block, args)),
            _ => None,
        }
    }
}

impl<V, R, C> IsCall for Action<V, R, C> {
    type Value = V;

    fn as_call(&self) -> Option<CallEffect<'_, Self::Value>> {
        match self {
            Action::Call(callee, args, results) => Some(CallEffect {
                callee: *callee,
                args,
                results,
            }),
            _ => None,
        }
    }
}

impl<V, R, C> IsReturn for Action<V, R, C> {
    type Value = V;

    fn as_return(&self) -> Option<&Self::Value> {
        match self {
            Action::Return(v) => Some(v),
            _ => None,
        }
    }
}

impl<V, R, C> IsYield for Action<V, R, C> {
    type Value = V;

    fn as_yield(&self) -> Option<&Self::Value> {
        match self {
            Action::Yield(v) => Some(v),
            _ => None,
        }
    }
}

impl<V, R, C> IsPush for Action<V, R, C> {
    type CursorEntry = C;

    #[allow(clippy::wrong_self_convention)]
    fn as_push(self) -> Option<Self::CursorEntry> {
        match self {
            Action::Push(c) => Some(c),
            _ => None,
        }
    }
}
