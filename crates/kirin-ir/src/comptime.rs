pub trait CompileTimeValue: Clone + std::fmt::Debug + std::hash::Hash + PartialEq {}

impl CompileTimeValue for i32 {}
impl CompileTimeValue for i64 {}
impl CompileTimeValue for u32 {}
impl CompileTimeValue for u64 {}
