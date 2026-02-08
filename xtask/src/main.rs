mod commands;

use std::process::ExitCode;

fn main() -> ExitCode {
    commands::run()
}
