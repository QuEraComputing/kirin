use crate::ExecutionSeed;

/// Shell-facing control returned after semantic effect consumption.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "controls must be handled by the interpreter shell"]
pub enum Control<Stop> {
    Advance,
    Stay,
    Push(ExecutionSeed),
    Replace(ExecutionSeed),
    Pop,
    Stop(Stop),
}

impl<S> Control<S> {
    pub fn map_stop<T>(self, f: impl FnOnce(S) -> T) -> Control<T> {
        match self {
            Control::Advance => Control::Advance,
            Control::Stay => Control::Stay,
            Control::Push(seed) => Control::Push(seed),
            Control::Replace(seed) => Control::Replace(seed),
            Control::Pop => Control::Pop,
            Control::Stop(stop) => Control::Stop(f(stop)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Control;

    #[test]
    fn map_stop_only_transforms_stop_payload() {
        let control = Control::Stop(4_u8).map_stop(u16::from);
        assert_eq!(control, Control::Stop(4_u16));

        assert_eq!(Control::<u8>::Advance.map_stop(u16::from), Control::Advance);
        assert_eq!(Control::<u8>::Stay.map_stop(u16::from), Control::Stay);
        assert_eq!(Control::<u8>::Pop.map_stop(u16::from), Control::Pop);
    }
}
