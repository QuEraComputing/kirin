#![allow(dead_code)]

use std::convert::Infallible;

use kirin_interpreter_3::Machine;

#[derive(Debug, Default)]
pub struct TestMachine;

impl Machine for TestMachine {
    type Effect = Infallible;
    type Error = Infallible;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<(), Self::Error> {
        match effect {}
    }
}
