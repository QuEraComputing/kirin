// Integration tests for Lexical/Lifted wrapper enums.
// Sub-module tests (ret::tests, call::tests) cover individual types.
// Here we verify that the #[wraps] delegation works correctly.

// Note: We cannot easily construct FunctionBody or Lambda from outside their
// modules (private fields without public constructors usable in tests).
// Tests focus on the Return and Call variants which have accessible constructors
// in their respective sub-module tests and can be wrapped here via the public enum.
