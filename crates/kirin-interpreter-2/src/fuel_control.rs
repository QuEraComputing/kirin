/// Shell-owned statement fuel control.
pub trait FuelControl {
    fn fuel(&self) -> Option<u64>;

    fn set_fuel(&mut self, fuel: Option<u64>);

    fn add_fuel(&mut self, fuel: u64) {
        let next = self.fuel().map(|current| current.saturating_add(fuel));
        self.set_fuel(next);
    }
}
