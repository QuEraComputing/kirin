/// Shell-owned host interrupt control.
pub trait InterruptControl {
    fn request_interrupt(&mut self);

    fn clear_interrupt(&mut self);

    fn interrupt_requested(&self) -> bool;
}
