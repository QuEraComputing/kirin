/// Shell-facing control returned after semantic effect consumption.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "controls must be handled by the interpreter shell"]
pub enum Shell<Stop, Seed = ()> {
    Advance,
    Stay,
    Push(Seed),
    Replace(Seed),
    Pop,
    Stop(Stop),
}

impl<S, Seed> Shell<S, Seed> {
    pub fn map_stop<T>(self, f: impl FnOnce(S) -> T) -> Shell<T, Seed> {
        match self {
            Shell::Advance => Shell::Advance,
            Shell::Stay => Shell::Stay,
            Shell::Push(seed) => Shell::Push(seed),
            Shell::Replace(seed) => Shell::Replace(seed),
            Shell::Pop => Shell::Pop,
            Shell::Stop(stop) => Shell::Stop(f(stop)),
        }
    }

    pub fn map_seed<T>(self, f: impl FnOnce(Seed) -> T) -> Shell<S, T> {
        match self {
            Shell::Advance => Shell::Advance,
            Shell::Stay => Shell::Stay,
            Shell::Push(seed) => Shell::Push(f(seed)),
            Shell::Replace(seed) => Shell::Replace(f(seed)),
            Shell::Pop => Shell::Pop,
            Shell::Stop(stop) => Shell::Stop(stop),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Shell;

    #[test]
    fn map_stop_only_transforms_stop_payload() {
        let control: Shell<u16> = Shell::Stop(4_u8).map_stop(u16::from);
        assert_eq!(control, Shell::Stop(4_u16));

        assert_eq!(
            Shell::<u8>::Advance.map_stop(u16::from),
            Shell::<u16>::Advance
        );
        assert_eq!(Shell::<u8>::Stay.map_stop(u16::from), Shell::<u16>::Stay);
        assert_eq!(Shell::<u8>::Pop.map_stop(u16::from), Shell::<u16>::Pop);
    }
}
