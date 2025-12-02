/// Arena ID
/// an ID object can only be created by
/// `arena.next_id()` or `arena.insert`
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Id(pub(crate) usize);

impl Id {
    /// return raw ID as usize
    pub fn raw(self) -> usize {
        self.0
    }
}
