use std::io::Write;
use std::process::{Command, Stdio};

pub fn rustfmt<S: AsRef<str>>(src: S) -> String {
    let src = src.as_ref();
    let Ok(mut child) = Command::new("rustfmt")
        .arg("--emit")
        .arg("stdout")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    else {
        return src.to_string();
    };

    let Some(stdin) = child.stdin.as_mut() else {
        return src.to_string();
    };
    if stdin.write_all(src.as_bytes()).is_err() {
        return src.to_string();
    }

    let Ok(output) = child.wait_with_output() else {
        return src.to_string();
    };
    if !output.status.success() {
        return src.to_string();
    }

    String::from_utf8(output.stdout).unwrap_or_else(|_| src.to_string())
}

pub fn rustfmt_display(value: &impl std::fmt::Display) -> String {
    rustfmt(value.to_string())
}
