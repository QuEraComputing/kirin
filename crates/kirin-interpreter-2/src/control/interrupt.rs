/// Shell-owned host interrupt control.
pub trait Interrupt {
    fn request_interrupt(&mut self);

    fn clear_interrupt(&mut self);

    fn interrupt_requested(&self) -> bool;
}
