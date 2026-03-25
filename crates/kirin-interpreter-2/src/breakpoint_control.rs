use kirin_ir::{CompileStage, Statement};

/// Public statement-oriented execution locations for shell breakpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionLocation {
    BeforeStatement(Statement),
    AfterStatement(Statement),
}

/// Shell-owned breakpoint keyed by stage and execution location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Breakpoint {
    stage: CompileStage,
    location: ExecutionLocation,
}

impl Breakpoint {
    pub const fn new(stage: CompileStage, location: ExecutionLocation) -> Self {
        Self { stage, location }
    }

    pub const fn stage(&self) -> CompileStage {
        self.stage
    }

    pub const fn location(&self) -> ExecutionLocation {
        self.location
    }
}

/// Shell-owned breakpoint set management.
pub trait BreakpointControl {
    fn add_breakpoint(&mut self, breakpoint: Breakpoint) -> bool;

    fn remove_breakpoint(&mut self, breakpoint: &Breakpoint) -> bool;

    fn has_breakpoint(&self, breakpoint: &Breakpoint) -> bool;
}
