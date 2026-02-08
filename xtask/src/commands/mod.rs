use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

mod new_rfc;
mod quick_validate;

const USAGE: &str = "\
Usage:
  cargo xtask quick-validate <skill_directory>
  cargo xtask new-rfc <title> [options]";

pub fn run() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        println!("{USAGE}");
        return ExitCode::from(1);
    };

    match command.as_str() {
        "quick-validate" => quick_validate::run(args.collect()),
        "new-rfc" => new_rfc::run(args.collect()),
        _ => {
            println!("{USAGE}");
            ExitCode::from(1)
        }
    }
}

pub(super) fn find_workspace_root() -> Result<PathBuf, String> {
    let start =
        env::current_dir().map_err(|error| format!("Failed to get current directory: {error}"))?;
    find_workspace_root_from(&start).ok_or_else(|| {
        format!(
            "Failed to find Kirin workspace root from '{}': no matching Cargo.toml found",
            start.display()
        )
    })
}

fn find_workspace_root_from(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let cargo_toml = dir.join("Cargo.toml");
        if !cargo_toml.is_file() {
            continue;
        }

        let Ok(content) = fs::read_to_string(&cargo_toml) else {
            continue;
        };
        if is_kirin_workspace_toml(&content) {
            return Some(dir.to_path_buf());
        }
    }

    None
}

fn is_kirin_workspace_toml(content: &str) -> bool {
    let has_workspace_table = content.lines().any(|line| line.trim() == "[workspace]");
    let has_kirin_package = content
        .lines()
        .any(|line| line.trim() == "name = \"kirin\"");
    let has_xtask_member = content.lines().any(|line| line.contains("\"xtask\""));
    has_workspace_table && has_kirin_package && has_xtask_member
}

#[cfg(test)]
mod tests {
    use super::is_kirin_workspace_toml;

    #[test]
    fn detects_kirin_workspace_manifest() {
        let manifest = r#"
[package]
name = "kirin"
version = "0.1.0"

[workspace]
members = ["xtask", "crates/kirin-ir"]
"#;
        assert!(is_kirin_workspace_toml(manifest));
    }

    #[test]
    fn rejects_non_root_manifests() {
        let crate_manifest = r#"
[package]
name = "kirin-ir"
version = "0.1.0"
"#;
        assert!(!is_kirin_workspace_toml(crate_manifest));
    }
}
