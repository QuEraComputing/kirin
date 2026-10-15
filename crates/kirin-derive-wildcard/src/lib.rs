extern crate proc_macro;

// Nothing uses the model or the parser until the macro's entry point exists.
#[allow(dead_code)]
mod model;
#[allow(dead_code)]
mod parse;
