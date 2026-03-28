use crate::control::Shell;

/// Thin structural semantic machine trait.
pub trait Machine<'ir> {
    type Effect;
    type Stop;
    type Seed;
}

/// Machine-owned effect consumption.
pub trait ConsumeEffect<'ir>: Machine<'ir> {
    type Error;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Shell<Self::Stop, Self::Seed>, Self::Error>;
}
