use std::{
    fs, io,
    path::Path,
    path::PathBuf,
    process::{Command, ExitCode},
};

use clap::{ArgGroup, CommandFactory, Parser, ValueEnum};
use tera::{Context, Tera};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::find_workspace_root;

const TEMPLATE_PATH: &str = "rfc/0000-template.md";
const DEFAULT_AUTHOR: &str = "unknown";

#[derive(Debug, Parser)]
#[command(name = "new-rfc", disable_help_subcommand = true)]
#[command(group(
    ArgGroup::new("title_source")
        .args(["title", "title_parts"])
        .required(true)
))]
struct NewRfcCli {
    /// Update existing RFC timestamp for this title.
    #[arg(long)]
    update: bool,
    /// RFC lifecycle status.
    #[arg(long, value_name = "status", ignore_case = true)]
    status: Option<RfcStatus>,
    /// Agent name that authored this RFC (for example: codex). Repeatable.
    #[arg(long = "agent", value_name = "name")]
    agents: Vec<String>,
    /// RFC author entry (repeatable).
    #[arg(long = "author", value_name = "author")]
    authors: Vec<String>,
    /// Discussion link or note.
    #[arg(long, value_name = "url-or-text")]
    discussion: Option<String>,
    /// Implementation tracking issue link or id.
    #[arg(long = "tracking-issue", value_name = "url-or-id")]
    tracking_issue: Option<String>,
    /// RFC ID this proposal depends on (repeatable).
    #[arg(long = "dependency", value_name = "rfc-id")]
    dependencies: Vec<String>,
    /// RFC ID this proposal supersedes (repeatable).
    #[arg(long, value_name = "rfc-id")]
    supersedes: Vec<String>,
    /// RFC ID that supersedes this proposal.
    #[arg(long = "superseded-by", value_name = "rfc-id")]
    superseded_by: Option<String>,
    /// Explicit RFC title.
    #[arg(long, value_name = "title", conflicts_with = "title_parts")]
    title: Option<String>,
    /// Positional RFC title parts.
    #[arg(value_name = "title", num_args = 1..)]
    title_parts: Vec<String>,
}

pub fn run(args: Vec<String>) -> ExitCode {
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h") {
        let mut command = NewRfcCli::command();
        print!("{}", command.render_long_help());
        println!();
        return ExitCode::SUCCESS;
    }

    let options = match parse_new_rfc_args(args) {
        Ok(options) => options,
        Err(message) => {
            println!("{message}");
            return ExitCode::from(1);
        }
    };

    match create_rfc_file(&options) {
        Ok(NewRfcOutcome::Created(path)) => {
            println!("[OK] Created RFC: {}", path.display());
            ExitCode::SUCCESS
        }
        Ok(NewRfcOutcome::Updated(id)) => {
            println!("RFC {id:04} has been updated, please go ahead edit the RFC");
            ExitCode::SUCCESS
        }
        Err(message) => {
            println!("{message}");
            ExitCode::from(1)
        }
    }
}

enum NewRfcOutcome {
    Created(PathBuf),
    Updated(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NewRfcOptions {
    title: String,
    update: bool,
    status: RfcStatus,
    status_overridden: bool,
    agents: Vec<String>,
    authors: Vec<String>,
    discussion: Option<String>,
    tracking_issue: Option<String>,
    dependencies: Option<Vec<String>>,
    supersedes: Option<Vec<String>>,
    superseded_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RfcStatus {
    #[value(name = "Draft")]
    Draft,
    #[value(name = "Review")]
    Review,
    #[value(name = "Accepted")]
    Accepted,
    #[value(name = "Rejected")]
    Rejected,
    #[value(name = "Implemented")]
    Implemented,
    #[value(name = "Superseded")]
    Superseded,
    #[value(name = "Withdrawn")]
    Withdrawn,
}

impl RfcStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Review => "Review",
            Self::Accepted => "Accepted",
            Self::Rejected => "Rejected",
            Self::Implemented => "Implemented",
            Self::Superseded => "Superseded",
            Self::Withdrawn => "Withdrawn",
        }
    }
}

impl NewRfcOptions {
    fn with_title(title: String) -> Self {
        Self {
            title,
            update: false,
            status: RfcStatus::Draft,
            status_overridden: false,
            agents: vec![],
            authors: vec![],
            discussion: None,
            tracking_issue: None,
            dependencies: None,
            supersedes: None,
            superseded_by: None,
        }
    }
}

fn parse_new_rfc_args(args: Vec<String>) -> Result<NewRfcOptions, String> {
    let cli =
        NewRfcCli::try_parse_from(std::iter::once("new-rfc".to_string()).chain(args.into_iter()))
            .map_err(|error| error.to_string())?;

    let title = cli.title.unwrap_or_else(|| cli.title_parts.join(" "));
    let title = non_empty(title, "title")?;

    let mut options = NewRfcOptions::with_title(title);
    options.update = cli.update;
    if let Some(value) = cli.status {
        options.status = value;
        options.status_overridden = true;
    }
    if !cli.agents.is_empty() {
        let mut normalized = Vec::with_capacity(cli.agents.len());
        for agent in cli.agents {
            normalized.push(non_empty(agent, "--agent")?);
        }
        options.agents = normalized;
    }
    if !cli.authors.is_empty() {
        let mut normalized = Vec::with_capacity(cli.authors.len());
        for author in cli.authors {
            normalized.push(non_empty(author, "--author")?);
        }
        options.authors = normalized;
    }
    if let Some(value) = cli.discussion {
        options.discussion = Some(non_empty(value, "--discussion")?);
    }
    if let Some(value) = cli.tracking_issue {
        options.tracking_issue = Some(non_empty(value, "--tracking-issue")?);
    }
    if !cli.dependencies.is_empty() {
        let mut normalized = Vec::with_capacity(cli.dependencies.len());
        for rfc in cli.dependencies {
            normalized.push(non_empty(rfc, "--dependency")?);
        }
        options.dependencies = Some(normalized);
    }
    if !cli.supersedes.is_empty() {
        let mut normalized = Vec::with_capacity(cli.supersedes.len());
        for rfc in cli.supersedes {
            normalized.push(non_empty(rfc, "--supersedes")?);
        }
        options.supersedes = Some(normalized);
    }
    if let Some(value) = cli.superseded_by {
        options.superseded_by = Some(non_empty(value, "--superseded-by")?);
    }

    Ok(options)
}

fn non_empty(value: String, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    Ok(trimmed.to_string())
}

fn create_rfc_file(options: &NewRfcOptions) -> Result<NewRfcOutcome, String> {
    let workspace_root = find_workspace_root()?;
    let template_path = workspace_root.join(TEMPLATE_PATH);
    let template = fs::read_to_string(&template_path).map_err(|error| {
        format!(
            "Failed to read RFC template '{}': {error}",
            template_path.display()
        )
    })?;

    let slug = slugify_title(&options.title);
    if slug.is_empty() {
        return Err("Title must include at least one ASCII letter or digit".to_string());
    }

    let rfc_dir = workspace_root.join("rfc");
    fs::create_dir_all(&rfc_dir).map_err(|error| {
        format!(
            "Failed to create RFC directory '{}': {error}",
            rfc_dir.display()
        )
    })?;

    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| format!("Failed to format timestamp: {error}"))?;

    let existing = existing_rfc_for_slug(&rfc_dir, &slug)
        .map_err(|error| format!("Failed to check existing RFCs: {error}"))?;
    if let Some(existing) = existing {
        if options.update {
            update_existing_rfc_metadata(
                &existing.path,
                &timestamp,
                options.status_overridden.then_some(options.status),
                &options.authors,
                &options.agents,
                options.dependencies.as_deref().unwrap_or(&[]),
            )?;
            return Ok(NewRfcOutcome::Updated(existing.id));
        }
        return Err(format!(
            "RFC title already exists. Use a different title or edit existing RFC.\n\
             Existing RFC: {}\n\
             Hint: if you meant to update metadata, run with --update (and optional flags like --status, --author, --agent, or --dependency).",
            existing.path.display()
        ));
    }
    if options.update {
        return Err("Cannot update RFC: no existing RFC found for the given title.".to_string());
    }

    let next_id =
        next_rfc_id(&rfc_dir).map_err(|error| format!("Failed to compute next RFC id: {error}"))?;
    let file_name = format!("{next_id:04}-{slug}.md");
    let output_path = rfc_dir.join(file_name);

    let authors = resolve_authors(&options.authors, &workspace_root);
    let mut options = options.clone();
    options.authors = authors;
    let content = render_template(&template, next_id, &options, &timestamp)?;

    fs::write(&output_path, content).map_err(|error| {
        format!(
            "Failed to write RFC file '{}': {error}",
            output_path.display()
        )
    })?;
    Ok(NewRfcOutcome::Created(output_path))
}

fn render_template(
    template: &str,
    rfc_id: u32,
    options: &NewRfcOptions,
    timestamp: &str,
) -> Result<String, String> {
    let mut context = Context::new();
    context.insert("rfc_id", &format!("{rfc_id:04}"));
    context.insert("title", &options.title);
    context.insert("title_toml", &toml_escape(&options.title));
    context.insert("timestamp", timestamp);
    context.insert("status", &toml_escape(options.status.as_str()));
    let discussion = options.discussion.as_deref().map(toml_escape);
    context.insert("discussion", &discussion);
    let tracking_issue = options.tracking_issue.as_deref().map(toml_escape);
    context.insert("tracking_issue", &tracking_issue);
    let dependencies = options
        .dependencies
        .as_ref()
        .map(|ids| ids.iter().map(|rfc| toml_escape(rfc)).collect::<Vec<_>>());
    context.insert("dependencies", &dependencies);
    let superseded_by = options.superseded_by.as_deref().map(toml_escape);
    context.insert("superseded_by", &superseded_by);

    let authors = options
        .authors
        .iter()
        .map(|author| toml_escape(author))
        .collect::<Vec<_>>();
    context.insert("authors", &authors);

    let agents = (!options.agents.is_empty()).then(|| {
        options
            .agents
            .iter()
            .map(|agent| toml_escape(agent))
            .collect::<Vec<_>>()
    });
    context.insert("agents", &agents);

    let supersedes = options
        .supersedes
        .as_ref()
        .map(|ids| ids.iter().map(|rfc| toml_escape(rfc)).collect::<Vec<_>>());
    context.insert("supersedes", &supersedes);

    Tera::one_off(template, &context, false)
        .map_err(|error| format!("Failed to render RFC template: {error}"))
}

fn resolve_authors(authors: &[String], workspace_root: &Path) -> Vec<String> {
    if !authors.is_empty() {
        return authors.to_vec();
    }

    let git_name = git_config_value(workspace_root, "user.name");
    let git_email = git_config_value(workspace_root, "user.email");
    match compose_author(git_name, git_email) {
        Some(author) => vec![author],
        None => vec![DEFAULT_AUTHOR.to_string()],
    }
}

fn git_config_value(workspace_root: &Path, key: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("config")
        .arg("--get")
        .arg(key)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn compose_author(name: Option<String>, email: Option<String>) -> Option<String> {
    match (name, email) {
        (Some(name), Some(email)) => Some(format!("{name} <{email}>")),
        (Some(name), None) => Some(name),
        (None, Some(email)) => Some(email),
        (None, None) => None,
    }
}

fn toml_escape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for char in value.chars() {
        match char {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            _ => output.push(char),
        }
    }
    output
}

fn next_rfc_id(rfc_dir: &Path) -> io::Result<u32> {
    let mut max_id = None;
    for entry in fs::read_dir(rfc_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some((id, _)) = file_name.split_once('-') else {
            continue;
        };
        if id.is_empty() || !id.chars().all(|char| char.is_ascii_digit()) {
            continue;
        }

        let Ok(id) = id.parse::<u32>() else {
            continue;
        };
        max_id = Some(max_id.map_or(id, |current_max: u32| current_max.max(id)));
    }

    match max_id {
        Some(id) => id
            .checked_add(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "RFC id overflow")),
        None => Ok(0),
    }
}

struct ExistingRfc {
    id: u32,
    path: PathBuf,
}

fn existing_rfc_for_slug(rfc_dir: &Path, slug: &str) -> io::Result<Option<ExistingRfc>> {
    for entry in fs::read_dir(rfc_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some((id, rest)) = file_name.split_once('-') else {
            continue;
        };
        if id.is_empty() || !id.chars().all(|char| char.is_ascii_digit()) {
            continue;
        }
        let Ok(id) = id.parse::<u32>() else {
            continue;
        };

        if rest == format!("{slug}.md") {
            return Ok(Some(ExistingRfc {
                id,
                path: entry.path(),
            }));
        }
    }

    Ok(None)
}

fn update_existing_rfc_metadata(
    path: &Path,
    timestamp: &str,
    new_status: Option<RfcStatus>,
    author_additions: &[String],
    agent_additions: &[String],
    dependency_additions: &[String],
) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read existing RFC '{}': {error}", path.display()))?;

    let had_trailing_newline = content.ends_with('\n');
    let mut lines = content
        .lines()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();

    let mut delimiter_indices = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (line.trim() == "+++").then_some(index));
    let Some(frontmatter_start) = delimiter_indices.next() else {
        return Err(format!(
            "Failed to update RFC '{}': missing TOML frontmatter start delimiter",
            path.display()
        ));
    };
    let Some(mut frontmatter_end) = delimiter_indices.next() else {
        return Err(format!(
            "Failed to update RFC '{}': missing TOML frontmatter end delimiter",
            path.display()
        ));
    };
    if frontmatter_start >= frontmatter_end {
        return Err(format!(
            "Failed to update RFC '{}': invalid TOML frontmatter boundaries",
            path.display()
        ));
    }

    let status_line = new_status.map(|status| format!("status = \"{}\"", status.as_str()));
    let mut last_updated_set = false;
    let mut status_set = status_line.is_none();
    let mut status_index = None;
    let mut authors_index = None;
    let mut agents_index = None;
    let mut dependencies_index = None;
    let mut legacy_agent_index = None;
    for (offset, line) in lines[(frontmatter_start + 1)..frontmatter_end]
        .iter_mut()
        .enumerate()
    {
        let index = frontmatter_start + 1 + offset;
        if line.trim_start().starts_with("last_updated = ") {
            *line = format!("last_updated = \"{timestamp}\"");
            last_updated_set = true;
            continue;
        }

        if line.trim_start().starts_with("status = ") {
            status_index = Some(index);
        }
        if line.trim_start().starts_with("authors = ") {
            authors_index = Some(index);
        }
        if line.trim_start().starts_with("agents = ") {
            agents_index = Some(index);
        }
        if line.trim_start().starts_with("dependencies = ") {
            dependencies_index = Some(index);
        }
        if line.trim_start().starts_with("agent = ") {
            legacy_agent_index = Some(index);
        }

        if let Some(status_line) = &status_line {
            if line.trim_start().starts_with("status = ") {
                *line = status_line.clone();
                status_set = true;
            }
        }
    }
    if !last_updated_set {
        return Err(format!(
            "Failed to update RFC '{}': missing last_updated field in TOML frontmatter",
            path.display()
        ));
    }
    if !status_set {
        return Err(format!(
            "Failed to update RFC '{}': missing status field in TOML frontmatter",
            path.display()
        ));
    }

    if !author_additions.is_empty() {
        if let Some(index) = authors_index {
            let mut authors =
                parse_toml_string_array_line("authors", &lines[index]).ok_or_else(|| {
                    format!(
                        "Failed to update RFC '{}': unable to parse authors field as TOML string array",
                        path.display()
                    )
                })?;
            append_unique(&mut authors, author_additions);
            lines[index] = format_toml_string_array_line("authors", &authors);
        } else {
            let insert_at = status_index.map_or(frontmatter_end, |index| index + 1);
            lines.insert(
                insert_at,
                format_toml_string_array_line("authors", author_additions),
            );
            if insert_at <= frontmatter_end {
                frontmatter_end += 1;
            }
            authors_index = Some(insert_at);
        }
    }

    if !agent_additions.is_empty() {
        if let Some(index) = agents_index {
            let mut agents =
                parse_toml_string_array_line("agents", &lines[index]).ok_or_else(|| {
                    format!(
                        "Failed to update RFC '{}': unable to parse agents field as TOML string array",
                        path.display()
                    )
                })?;
            append_unique(&mut agents, agent_additions);
            lines[index] = format_toml_string_array_line("agents", &agents);
        } else if let Some(index) = legacy_agent_index {
            let mut agents = parse_toml_string_line("agent", &lines[index])
                .map(|agent| vec![agent])
                .ok_or_else(|| {
                    format!(
                        "Failed to update RFC '{}': unable to parse agent field as TOML string",
                        path.display()
                    )
                })?;
            append_unique(&mut agents, agent_additions);
            lines[index] = format_toml_string_array_line("agents", &agents);
        } else {
            let insert_at = authors_index
                .map(|index| index + 1)
                .or_else(|| status_index.map(|index| index + 1))
                .unwrap_or(frontmatter_end);
            lines.insert(
                insert_at,
                format_toml_string_array_line("agents", agent_additions),
            );
            if insert_at <= frontmatter_end {
                frontmatter_end += 1;
            }
        }
    }

    if !dependency_additions.is_empty() {
        if let Some(index) = dependencies_index {
            let mut dependencies =
                parse_toml_string_array_line("dependencies", &lines[index]).ok_or_else(|| {
                    format!(
                        "Failed to update RFC '{}': unable to parse dependencies field as TOML string array",
                        path.display()
                    )
                })?;
            append_unique(&mut dependencies, dependency_additions);
            lines[index] = format_toml_string_array_line("dependencies", &dependencies);
        } else {
            let insert_at = frontmatter_end;
            lines.insert(
                insert_at,
                format_toml_string_array_line("dependencies", dependency_additions),
            );
        }
    }

    let mut rewritten = lines.join("\n");
    if had_trailing_newline {
        rewritten.push('\n');
    }
    fs::write(path, rewritten)
        .map_err(|error| format!("Failed to write existing RFC '{}': {error}", path.display()))
}

fn append_unique(existing: &mut Vec<String>, additions: &[String]) {
    for addition in additions {
        if !existing.iter().any(|value| value == addition) {
            existing.push(addition.clone());
        }
    }
}

fn format_toml_string_array_line(key: &str, values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("\"{}\"", toml_escape(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{key} = [{values}]")
}

fn parse_toml_string_line(key: &str, line: &str) -> Option<String> {
    let trimmed = line.trim();
    let prefix = format!("{key} =");
    let value = trimmed.strip_prefix(&prefix)?.trim();
    parse_toml_string(value)
}

fn parse_toml_string_array_line(key: &str, line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    let prefix = format!("{key} =");
    let value = trimmed.strip_prefix(&prefix)?.trim();
    parse_toml_string_array(value)
}

fn parse_toml_string_array(value: &str) -> Option<Vec<String>> {
    let trimmed = value.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }

    let mut values = vec![];
    let mut rest = trimmed[1..trimmed.len() - 1].trim();
    if rest.is_empty() {
        return Some(values);
    }

    while !rest.is_empty() {
        let (item, next) = take_toml_string(rest)?;
        values.push(item);
        let next = next.trim_start();
        if next.is_empty() {
            break;
        }
        let remainder = next.strip_prefix(',')?;
        rest = remainder.trim_start();
    }

    Some(values)
}

fn take_toml_string(value: &str) -> Option<(String, &str)> {
    let value = value.trim_start();
    if !value.starts_with('"') {
        return None;
    }

    let mut result = String::new();
    let mut escaped = false;
    for (index, ch) in value.char_indices().skip(1) {
        if escaped {
            let decoded = match ch {
                '\\' => '\\',
                '"' => '"',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            };
            result.push(decoded);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Some((result, &value[index + ch.len_utf8()..])),
            other => result.push(other),
        }
    }

    None
}

fn parse_toml_string(value: &str) -> Option<String> {
    let (parsed, rest) = take_toml_string(value)?;
    if rest.trim().is_empty() {
        Some(parsed)
    } else {
        None
    }
}

fn slugify_title(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut previous_is_dash = false;

    for char in title.chars() {
        let char = char.to_ascii_lowercase();
        if char.is_ascii_alphanumeric() {
            slug.push(char);
            previous_is_dash = false;
            continue;
        }

        if !previous_is_dash {
            slug.push('-');
            previous_is_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        NewRfcOptions, RfcStatus, compose_author, existing_rfc_for_slug, next_rfc_id,
        parse_new_rfc_args, render_template, resolve_authors, slugify_title, toml_escape,
        update_existing_rfc_metadata,
    };

    #[test]
    fn slugify_title_normalizes_text() {
        assert_eq!(slugify_title("The Zen of Kirin"), "the-zen-of-kirin");
        assert_eq!(
            slugify_title("  RFC: parser/pretty roundtrip  "),
            "rfc-parser-pretty-roundtrip"
        );
        assert_eq!(slugify_title("----"), "");
    }

    #[test]
    fn next_rfc_id_uses_max_existing_id() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "xtask-new-rfc-id-test-{}-{unique}",
            std::process::id()
        ));

        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(temp_dir.join("0000-alpha.md"), "").expect("seed file");
        fs::write(temp_dir.join("0004-beta.md"), "").expect("seed file");
        fs::write(temp_dir.join("abc.md"), "").expect("ignored file");
        fs::write(temp_dir.join("0099-gamma.md"), "").expect("seed file");

        let next = next_rfc_id(&temp_dir).expect("next id should be computed");
        assert_eq!(next, 100);

        fs::remove_dir_all(temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn existing_rfc_for_slug_detects_duplicate() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "xtask-rfc-dup-test-{}-{unique}",
            std::process::id()
        ));

        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(temp_dir.join("0000-template.md"), "").expect("seed file");
        fs::write(temp_dir.join("0001-the-zen-of-kirin.md"), "").expect("seed file");

        let existing =
            existing_rfc_for_slug(&temp_dir, "the-zen-of-kirin").expect("scan should work");
        assert!(existing.as_ref().is_some_and(|entry| entry.id == 1));

        fs::remove_dir_all(temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn render_template_replaces_auto_fields() {
        let options = NewRfcOptions::with_title("The Zen of Kirin".to_string());
        let rendered = render_template(
            "rfc={{ rfc_id }} title={{ title }} created={{ timestamp }}",
            12,
            &options,
            "2026-02-08T00:00:00Z",
        )
        .expect("template should render");
        assert_eq!(
            rendered,
            "rfc=0012 title=The Zen of Kirin created=2026-02-08T00:00:00Z"
        );
    }

    #[test]
    fn render_template_includes_dependencies_when_set() {
        let mut options = NewRfcOptions::with_title("Has deps".to_string());
        options.dependencies = Some(vec!["0001".to_string(), "0003".to_string()]);
        let rendered = render_template(
            "{% if dependencies %}dependencies = [{% for rfc in dependencies %}\"{{ rfc }}\"{% if not loop.last %}, {% endif %}{% endfor %}]{% endif %}",
            1,
            &options,
            "2026-02-08T00:00:00Z",
        )
        .expect("template should render");

        assert_eq!(rendered, "dependencies = [\"0001\", \"0003\"]");
    }

    #[test]
    fn toml_escape_escapes_quotes_and_backslashes() {
        assert_eq!(toml_escape("a\"b\\c"), "a\\\"b\\\\c");
    }

    #[test]
    fn parse_args_supports_positional_title_and_options() {
        let options = parse_new_rfc_args(vec![
            "metadata".to_string(),
            "tracking".to_string(),
            "--update".to_string(),
            "--status".to_string(),
            "Review".to_string(),
            "--agent".to_string(),
            "codex".to_string(),
            "--author".to_string(),
            "alice".to_string(),
            "--author".to_string(),
            "bob".to_string(),
            "--tracking-issue".to_string(),
            "https://example.com/issues/1".to_string(),
            "--dependency".to_string(),
            "0001".to_string(),
            "--dependency".to_string(),
            "0003".to_string(),
            "--supersedes".to_string(),
            "0007".to_string(),
        ])
        .expect("args should parse");

        assert_eq!(options.title, "metadata tracking");
        assert!(options.update);
        assert_eq!(options.status, RfcStatus::Review);
        assert!(options.status_overridden);
        assert_eq!(options.agents, vec!["codex".to_string()]);
        assert_eq!(options.authors, vec!["alice", "bob"]);
        assert_eq!(
            options.tracking_issue,
            Some("https://example.com/issues/1".to_string())
        );
        assert_eq!(
            options.dependencies,
            Some(vec!["0001".to_string(), "0003".to_string()])
        );
        assert_eq!(options.supersedes, Some(vec!["0007".to_string()]));
    }

    #[test]
    fn parse_args_rejects_mixed_title_modes() {
        let error = parse_new_rfc_args(vec![
            "--title".to_string(),
            "explicit".to_string(),
            "positional".to_string(),
        ])
        .expect_err("parser should reject mixed title input");

        assert!(error.contains("the argument '--title <title>' cannot be used with '[title]...'"));
    }

    #[test]
    fn parse_args_defaults_optional_metadata_to_none() {
        let options = parse_new_rfc_args(vec!["minimal".to_string()]).expect("args should parse");
        assert!(!options.update);
        assert_eq!(options.status, RfcStatus::Draft);
        assert!(!options.status_overridden);
        assert!(options.agents.is_empty());
        assert_eq!(options.discussion, None);
        assert_eq!(options.tracking_issue, None);
        assert_eq!(options.dependencies, None);
        assert_eq!(options.supersedes, None);
        assert_eq!(options.superseded_by, None);
    }

    #[test]
    fn parse_args_accepts_lowercase_status() {
        let options = parse_new_rfc_args(vec![
            "lowercase status".to_string(),
            "--status".to_string(),
            "implemented".to_string(),
        ])
        .expect("lowercase status should parse");

        assert_eq!(options.status, RfcStatus::Implemented);
        assert!(options.status_overridden);
    }

    #[test]
    fn parse_args_rejects_invalid_status() {
        let error = parse_new_rfc_args(vec![
            "invalid status".to_string(),
            "--status".to_string(),
            "Done".to_string(),
        ])
        .expect_err("parser should reject unsupported status");

        assert!(error.contains("invalid value 'Done' for '--status <status>'"));
        assert!(error.contains(
            "[possible values: Draft, Review, Accepted, Rejected, Implemented, Superseded, Withdrawn]"
        ));
    }

    #[test]
    fn compose_author_prefers_name_and_email() {
        let author = compose_author(Some("Alice".to_string()), Some("a@example.com".to_string()));
        assert_eq!(author, Some("Alice <a@example.com>".to_string()));
    }

    #[test]
    fn compose_author_handles_name_only() {
        let author = compose_author(Some("Alice".to_string()), None);
        assert_eq!(author, Some("Alice".to_string()));
    }

    #[test]
    fn resolve_authors_keeps_explicit_values() {
        let input = vec!["alice".to_string(), "bob".to_string()];
        let resolved = resolve_authors(&input, Path::new("."));
        assert_eq!(resolved, input);
    }

    #[test]
    fn render_template_omits_optional_metadata_when_none() {
        let options = NewRfcOptions::with_title("No optional fields".to_string());
        let rendered = render_template(
            "{% if agents %}agents = [{% for agent in agents %}\"{{ agent }}\"{% if not loop.last %}, {% endif %}{% endfor %}]{% endif %}\n\
             {% if discussion %}discussion = \"{{ discussion }}\"{% endif %}\n\
             {% if tracking_issue %}tracking_issue = \"{{ tracking_issue }}\"{% endif %}\n\
             {% if dependencies %}dependencies = [{% for rfc in dependencies %}\"{{ rfc }}\"{% if not loop.last %}, {% endif %}{% endfor %}]{% endif %}\n\
             {% if supersedes %}supersedes = [{% for rfc in supersedes %}\"{{ rfc }}\"{% if not loop.last %}, {% endif %}{% endfor %}]{% endif %}\n\
             {% if superseded_by %}superseded_by = \"{{ superseded_by }}\"{% endif %}",
            1,
            &options,
            "2026-02-08T00:00:00Z",
        )
        .expect("template should render");

        assert!(!rendered.contains("agents ="));
        assert!(!rendered.contains("discussion ="));
        assert!(!rendered.contains("tracking_issue ="));
        assert!(!rendered.contains("dependencies ="));
        assert!(!rendered.contains("supersedes ="));
        assert!(!rendered.contains("superseded_by ="));
    }

    #[test]
    fn update_existing_rfc_metadata_rewrites_last_updated_field() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let temp_file = std::env::temp_dir().join(format!(
            "xtask-rfc-update-last-updated-{}-{unique}.md",
            std::process::id()
        ));
        let original = "\
+++
rfc = \"0001\"
last_updated = \"2026-01-01T00:00:00Z\"
+++

# RFC
";
        fs::write(&temp_file, original).expect("temp rfc should be written");

        update_existing_rfc_metadata(&temp_file, "2026-02-08T00:00:00Z", None, &[], &[], &[])
            .expect("timestamp should update");
        let updated = fs::read_to_string(&temp_file).expect("updated rfc should be readable");
        assert!(updated.contains("last_updated = \"2026-02-08T00:00:00Z\""));

        fs::remove_file(temp_file).expect("temp rfc should be removed");
    }

    #[test]
    fn update_existing_rfc_metadata_updates_status_when_requested() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let temp_file = std::env::temp_dir().join(format!(
            "xtask-rfc-update-status-{}-{unique}.md",
            std::process::id()
        ));
        let original = "\
+++
rfc = \"0002\"
status = \"Draft\"
last_updated = \"2026-01-01T00:00:00Z\"
+++

# RFC
";
        fs::write(&temp_file, original).expect("temp rfc should be written");

        update_existing_rfc_metadata(
            &temp_file,
            "2026-02-08T00:00:00Z",
            Some(RfcStatus::Implemented),
            &[],
            &[],
            &[],
        )
        .expect("status and timestamp should update");
        let updated = fs::read_to_string(&temp_file).expect("updated rfc should be readable");
        assert!(updated.contains("status = \"Implemented\""));
        assert!(updated.contains("last_updated = \"2026-02-08T00:00:00Z\""));

        fs::remove_file(temp_file).expect("temp rfc should be removed");
    }

    #[test]
    fn update_existing_rfc_metadata_adds_agents_when_requested() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let temp_file = std::env::temp_dir().join(format!(
            "xtask-rfc-update-agent-{}-{unique}.md",
            std::process::id()
        ));
        let original = "\
+++
rfc = \"0003\"
status = \"Draft\"
last_updated = \"2026-01-01T00:00:00Z\"
+++

# RFC
";
        fs::write(&temp_file, original).expect("temp rfc should be written");

        let agent_additions = vec!["codex".to_string()];
        update_existing_rfc_metadata(
            &temp_file,
            "2026-02-08T00:00:00Z",
            None,
            &[],
            &agent_additions,
            &[],
        )
        .expect("agents and timestamp should update");
        let updated = fs::read_to_string(&temp_file).expect("updated rfc should be readable");
        assert!(updated.contains("agents = [\"codex\"]"));
        assert!(updated.contains("last_updated = \"2026-02-08T00:00:00Z\""));

        fs::remove_file(temp_file).expect("temp rfc should be removed");
    }

    #[test]
    fn update_existing_rfc_metadata_adds_dependencies_when_requested() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let temp_file = std::env::temp_dir().join(format!(
            "xtask-rfc-update-dependency-{}-{unique}.md",
            std::process::id()
        ));
        let original = "\
+++
rfc = \"0004\"
status = \"Draft\"
last_updated = \"2026-01-01T00:00:00Z\"
+++

# RFC
";
        fs::write(&temp_file, original).expect("temp rfc should be written");

        let dependency_additions = vec!["0002".to_string()];
        update_existing_rfc_metadata(
            &temp_file,
            "2026-02-08T00:00:00Z",
            None,
            &[],
            &[],
            &dependency_additions,
        )
        .expect("dependencies and timestamp should update");
        let updated = fs::read_to_string(&temp_file).expect("updated rfc should be readable");
        assert!(updated.contains("dependencies = [\"0002\"]"));
        assert!(updated.contains("last_updated = \"2026-02-08T00:00:00Z\""));

        fs::remove_file(temp_file).expect("temp rfc should be removed");
    }

    #[test]
    fn update_existing_rfc_metadata_appends_authors_and_agents() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let temp_file = std::env::temp_dir().join(format!(
            "xtask-rfc-update-lists-{}-{unique}.md",
            std::process::id()
        ));
        let original = "\
+++
rfc = \"0004\"
status = \"Review\"
authors = [\"alice\"]
agents = [\"codex\"]
dependencies = [\"0001\"]
last_updated = \"2026-01-01T00:00:00Z\"
+++

# RFC
";
        fs::write(&temp_file, original).expect("temp rfc should be written");

        let author_additions = vec!["alice".to_string(), "bob".to_string()];
        let agent_additions = vec!["codex".to_string(), "cursor".to_string()];
        let dependency_additions = vec!["0001".to_string(), "0003".to_string()];
        update_existing_rfc_metadata(
            &temp_file,
            "2026-02-08T00:00:00Z",
            None,
            &author_additions,
            &agent_additions,
            &dependency_additions,
        )
        .expect("list metadata and timestamp should update");
        let updated = fs::read_to_string(&temp_file).expect("updated rfc should be readable");
        assert!(updated.contains("authors = [\"alice\", \"bob\"]"));
        assert!(updated.contains("agents = [\"codex\", \"cursor\"]"));
        assert!(updated.contains("dependencies = [\"0001\", \"0003\"]"));
        assert!(updated.contains("last_updated = \"2026-02-08T00:00:00Z\""));

        fs::remove_file(temp_file).expect("temp rfc should be removed");
    }
}
