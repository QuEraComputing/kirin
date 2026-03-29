/// Thin structural semantic machine trait.
pub trait Machine<'ir> {
    type Effect;
    type Stop;
    type Seed;
}

/// Machine-owned effect consumption.
///
/// The `Output` type parameter is the result of consuming an effect:
/// - Inner machines produce directives: `ConsumeEffect<'ir, Directive<Stop, Seed>>`
/// - The interpreter shell is terminal: `ConsumeEffect<'ir, ()>`
pub trait ConsumeEffect<'ir, Output = ()>: Machine<'ir> {
    type Error;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Output, Self::Error>;
}
