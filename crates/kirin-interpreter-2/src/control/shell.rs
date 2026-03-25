use crate::seed::ExecutionSeed;

/// Shell-facing control returned after semantic effect consumption.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "controls must be handled by the interpreter shell"]
pub enum Shell<Stop> {
    Advance,
    Stay,
    Push(ExecutionSeed),
    Replace(ExecutionSeed),
    Pop,
    Stop(Stop),
}

impl<S> Shell<S> {
    pub fn map_stop<T>(self, f: impl FnOnce(S) -> T) -> Shell<T> {
        match self {
            Shell::Advance => Shell::Advance,
            Shell::Stay => Shell::Stay,
            Shell::Push(seed) => Shell::Push(seed),
            Shell::Replace(seed) => Shell::Replace(seed),
            Shell::Pop => Shell::Pop,
            Shell::Stop(stop) => Shell::Stop(f(stop)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Shell;

    #[test]
    fn map_stop_only_transforms_stop_payload() {
        let control = Shell::Stop(4_u8).map_stop(u16::from);
        assert_eq!(control, Shell::Stop(4_u16));

        assert_eq!(Shell::<u8>::Advance.map_stop(u16::from), Shell::Advance);
        assert_eq!(Shell::<u8>::Stay.map_stop(u16::from), Shell::Stay);
        assert_eq!(Shell::<u8>::Pop.map_stop(u16::from), Shell::Pop);
    }
}
