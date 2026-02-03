mod input;

pub use input::{InputMeta, PathBuilder};

// Re-export deprecated aliases for backwards compatibility
#[allow(deprecated)]
pub use input::{InputBuilder, InputContext};
