use crate::Control;

/// Thin structural semantic machine trait.
pub trait Machine<'ir> {
    type Effect;
    type Stop;
}

/// Machine-owned effect consumption.
pub trait ConsumeEffect<'ir>: Machine<'ir> {
    type Error;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Control<Self::Stop>, Self::Error>;
}
