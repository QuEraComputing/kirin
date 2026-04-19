// Re-export the cursor types from kirin-interpreter-9 for use by composed languages.
pub use kirin_interpreter_9::scf_cursor::{
    AbstractForCursor, AbstractIfCursor, AbstractSCFCursor, ForCursor, IfCursor, SCFCursor,
};

// ForLoopValue from interpreter-9 (scf_cursor::ForLoopValue) is implemented
// by dialect users at the composed-language level, where both local ForLoopValue
// and IF9ForLoopValue are in scope. The bridge between kirin-scf's ForLoopValue
// and interpreter-9's scf_cursor::ForLoopValue is provided via a blanket impl
// in each composed language's interpreter9 module (not here, to avoid orphan violations).
