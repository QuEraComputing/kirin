use proc_macro2::TokenStream;

/// Format a `TokenStream` through `rustfmt` for readable snapshot comparisons.
///
/// Falls back to `TokenStream::to_string()` if `rustfmt` is unavailable.
pub fn rustfmt_tokens(tokens: &TokenStream) -> String {
    let src = tokens.to_string();
    use std::io::Write;
    use std::process::{Command, Stdio};

    if let Ok(mut child) = Command::new("rustfmt")
        .arg("--emit")
        .arg("stdout")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(src.as_bytes());
        }
        if let Ok(output) = child.wait_with_output() {
            if output.status.success() {
                if let Ok(formatted) = String::from_utf8(output.stdout) {
                    return formatted;
                }
            }
        }
    }
    src
}
