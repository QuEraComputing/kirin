/// Framework-owned suspension reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Suspension {
    Breakpoint,
    FuelExhausted,
    HostInterrupt,
}
