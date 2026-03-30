pub trait Machine {
    type Effect;
    type Error;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error>;
}
