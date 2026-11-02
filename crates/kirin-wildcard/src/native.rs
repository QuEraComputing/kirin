use std::any::{Any, type_name};
use std::fmt::{self, Debug};
use std::rc::Rc;

use kirin::ir::{RewriteError, Rewriter, SSAValue, StageInfo};

/// A value passed to or returned by a native function: an [`SSAValue`] or
/// any other `'static` type, such as an `i64` one check computes and another
/// reads.
///
/// Pattern values bound by a match reach native functions as SSA values. A
/// `wildcard.constraint` tests a `bool`, and a `replace` needs SSA values on
/// both sides; anything else only travels between native functions, so each
/// dialect's functions can pass their own types without a shared enum.
///
/// ```
/// # use kirin_wildcard::NativeValue;
/// let value = NativeValue::new(42_i64);
/// assert_eq!(value.get::<i64>(), Some(&42));
/// assert_eq!(value.get::<bool>(), None);
///
/// // Wrapping a `NativeValue` again keeps what it holds readable.
/// assert_eq!(NativeValue::new(value).get::<i64>(), Some(&42));
/// ```
#[derive(Clone)]
pub struct NativeValue(Repr);

/// SSA values are bound for every hole of every match attempt, so they are
/// kept unboxed; other values only come from native calls.
#[derive(Clone)]
enum Repr {
    Ssa(SSAValue),
    Other(Rc<dyn NativeAny>),
}

/// What a [`NativeValue`] can hold besides an SSA value. `Debug` lets errors
/// show the value.
trait NativeAny: Any + Debug {}

impl<T: Any + Debug> NativeAny for T {}

impl NativeValue {
    /// Wrap `value`. An [`SSAValue`] is stored as one, so `replace` can use it,
    /// and a `NativeValue` is returned as is rather than wrapped again, which
    /// would hide what it holds.
    pub fn new<T: Any + Debug>(value: T) -> Self {
        let any = &value as &dyn Any;
        if let Some(&ssa) = any.downcast_ref::<SSAValue>() {
            return Self(Repr::Ssa(ssa));
        }
        if let Some(native) = any.downcast_ref::<NativeValue>() {
            return native.clone();
        }
        Self(Repr::Other(Rc::new(value)))
    }

    /// The value, if it is a `T`.
    pub fn get<T: Any>(&self) -> Option<&T> {
        match &self.0 {
            Repr::Ssa(ssa) => (ssa as &dyn Any).downcast_ref(),
            Repr::Other(value) => (value.as_ref() as &dyn Any).downcast_ref(),
        }
    }

    /// The SSA value, if this is one.
    pub fn ssa(&self) -> Option<SSAValue> {
        match self.0 {
            Repr::Ssa(ssa) => Some(ssa),
            Repr::Other(_) => None,
        }
    }

    /// Argument `position` of a native call, as a `T`.
    ///
    /// Fails with [`NativeError::Argument`] if it is missing or another type,
    /// which is a mistake in the rule rather than a failed match.
    pub fn arg<T: Any + Clone>(args: &[NativeValue], position: usize) -> Result<T, NativeError> {
        let value = args.get(position);
        value
            .and_then(|value| value.get::<T>())
            .cloned()
            .ok_or_else(|| NativeError::Argument {
                position,
                expected: type_name::<T>(),
                found: value.map(|value| format!("{value:?}")),
            })
    }
}

impl From<SSAValue> for NativeValue {
    fn from(value: SSAValue) -> Self {
        Self(Repr::Ssa(value))
    }
}

impl Debug for NativeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Repr::Ssa(ssa) => ssa.fmt(f),
            Repr::Other(value) => value.fmt(f),
        }
    }
}

/// Why a native function could not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeError {
    /// A native function was given an argument of the wrong type.
    ///
    /// - `position`: which argument, counting from 0
    /// - `expected`: the type the function takes, e.g. `"i64"`
    /// - `found`: the value it was given, printed with `Debug`, or `None` if
    ///   the call has fewer arguments than that
    Argument {
        position: usize,
        expected: &'static str,
        found: Option<String>,
    },
    /// An action's edit was rejected by the [`Rewriter`].
    Rewrite(RewriteError),
}

impl From<RewriteError> for NativeError {
    fn from(error: RewriteError) -> Self {
        NativeError::Rewrite(error)
    }
}

/// A native function called while matching. It reads the IR and returns the
/// results of its `native_call`, or `Ok(None)` if the match should fail,
/// e.g. because an operand is not the constant it needs.
///
/// Shared, so a checked rule can hold it at each call site.
pub type CheckFn<L> =
    Rc<dyn Fn(&StageInfo<L>, &[NativeValue]) -> Result<Option<Vec<NativeValue>>, NativeError>>;

/// A native function called while rewriting. It may edit the IR through the
/// pass's [`Rewriter`] and returns the results of its `native_call`.
///
/// Shared, so a checked rule can hold it at each call site.
pub type ActionFn<L> =
    Rc<dyn Fn(&mut Rewriter<'_, L>, &[NativeValue]) -> Result<Vec<NativeValue>, NativeError>>;
