/// Thin structural semantic machine trait.
pub trait Machine<'ir> {
    type Effect;
    type Stop;
    type Seed;
}

/// Machine-owned effect consumption.
pub trait ConsumeEffect<'ir>: Machine<'ir> {
    type Output;
    type Error;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Self::Output, Self::Error>;
}
