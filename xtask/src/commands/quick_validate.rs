use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use super::find_workspace_root;

const MAX_SKILL_NAME_LENGTH: usize = 64;
const USAGE: &str = "Usage: cargo xtask quick-validate <skill_directory>";
const ALLOWED_PROPERTIES: [&str; 5] = [
    "name",
    "description",
    "license",
    "allowed-tools",
    "metadata",
];

pub fn run(args: Vec<String>) -> ExitCode {
    if args.len() != 1 {
        println!("{USAGE}");
        return ExitCode::from(1);
    }

    let skill_path = resolve_skill_path(&args[0]);
    let (valid, message) = validate_skill(&skill_path);
    println!("{message}");
    if valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn resolve_skill_path(input: &str) -> PathBuf {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        return path;
    }

    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let from_cwd = cwd.join(&path);
    if from_cwd.exists() {
        return from_cwd;
    }

    let Ok(workspace_root) = find_workspace_root() else {
        return path;
    };
    workspace_root.join(path)
}

fn validate_skill(skill_path: &Path) -> (bool, String) {
    let skill_md = skill_path.join("SKILL.md");
    if !skill_md.exists() {
        return (false, "SKILL.md not found".to_string());
    }

    let content = match fs::read_to_string(&skill_md) {
        Ok(content) => content,
        Err(error) => return (false, format!("Failed to read SKILL.md: {error}")),
    };

    if !content.starts_with("---") {
        return (false, "No YAML frontmatter found".to_string());
    }

    let Some(frontmatter_text) = extract_frontmatter(&content) else {
        return (false, "Invalid frontmatter format".to_string());
    };

    let frontmatter = match serde_yaml::from_str::<serde_yaml::Value>(frontmatter_text) {
        Ok(frontmatter) => frontmatter,
        Err(error) => return (false, format!("Invalid YAML in frontmatter: {error}")),
    };

    let Some(frontmatter) = frontmatter.as_mapping() else {
        return (false, "Frontmatter must be a YAML dictionary".to_string());
    };

    let allowed: BTreeSet<&str> = ALLOWED_PROPERTIES.into_iter().collect();
    let mut unexpected = frontmatter
        .keys()
        .map(yaml_key_as_string)
        .filter(|key| !allowed.contains(key.as_str()))
        .collect::<Vec<_>>();
    unexpected.sort();

    if !unexpected.is_empty() {
        let allowed = ALLOWED_PROPERTIES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        let unexpected = unexpected.join(", ");
        return (
            false,
            format!(
                "Unexpected key(s) in SKILL.md frontmatter: {unexpected}. Allowed properties are: {allowed}"
            ),
        );
    }

    let Some(name) = frontmatter.get("name") else {
        return (false, "Missing 'name' in frontmatter".to_string());
    };
    let Some(description) = frontmatter.get("description") else {
        return (false, "Missing 'description' in frontmatter".to_string());
    };

    let Some(name) = name.as_str() else {
        return (
            false,
            format!("Name must be a string, got {}", python_type_name(name)),
        );
    };
    let name = name.trim();
    if !name.is_empty() {
        if !name
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '-')
        {
            return (
                false,
                format!(
                    "Name '{name}' should be hyphen-case (lowercase letters, digits, and hyphens only)"
                ),
            );
        }
        if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
            return (
                false,
                format!(
                    "Name '{name}' cannot start/end with hyphen or contain consecutive hyphens"
                ),
            );
        }
        if name.chars().count() > MAX_SKILL_NAME_LENGTH {
            return (
                false,
                format!(
                    "Name is too long ({} characters). Maximum is {} characters.",
                    name.chars().count(),
                    MAX_SKILL_NAME_LENGTH
                ),
            );
        }
    }

    let Some(description) = description.as_str() else {
        return (
            false,
            format!(
                "Description must be a string, got {}",
                python_type_name(description)
            ),
        );
    };
    let description = description.trim();
    if !description.is_empty() {
        if description.contains('<') || description.contains('>') {
            return (
                false,
                "Description cannot contain angle brackets (< or >)".to_string(),
            );
        }
        if description.chars().count() > 1024 {
            return (
                false,
                format!(
                    "Description is too long ({} characters). Maximum is 1024 characters.",
                    description.chars().count()
                ),
            );
        }
    }

    (true, "Skill is valid!".to_string())
}

fn extract_frontmatter(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

fn yaml_key_as_string(key: &serde_yaml::Value) -> String {
    match key {
        serde_yaml::Value::String(string) => string.clone(),
        _ => serde_yaml::to_string(key)
            .unwrap_or_else(|_| format!("{key:?}"))
            .trim()
            .to_string(),
    }
}

fn python_type_name(value: &serde_yaml::Value) -> &'static str {
    match value {
        serde_yaml::Value::Null => "NoneType",
        serde_yaml::Value::Bool(_) => "bool",
        serde_yaml::Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                "int"
            } else {
                "float"
            }
        }
        serde_yaml::Value::String(_) => "str",
        serde_yaml::Value::Sequence(_) => "list",
        serde_yaml::Value::Mapping(_) => "dict",
        serde_yaml::Value::Tagged(_) => "dict",
    }
}
