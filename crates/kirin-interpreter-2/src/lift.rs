/// Embeds a value from a smaller type into a larger type.
///
/// Used for both effect lifting (dialect effect -> machine effect) and
/// stop lifting (sub-machine stop -> composite stop). Defined on the source
/// type (like `Into`), so conversion logic lives with the effect type,
/// not the machine.
///
/// Replaces `LiftEffect<'ir, Sub>` and `LiftStop<'ir, Sub>`.
pub trait Lift<Target> {
    fn lift(self) -> Target;
}

/// Identity — any type lifts into itself.
impl<T> Lift<T> for T {
    fn lift(self) -> T {
        self
    }
}
