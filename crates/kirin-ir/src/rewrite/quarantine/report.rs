//! Rendering of the quarantine diagnostic report.

use std::fmt::{self, Write as _};

use crate::{Dialect, MutationEvent, StageInfo};

/// A flat dump of every arena id, followed by the mutation events.
///
/// Deliberately does not use the pretty-printer or the linked-list iterators:
/// those trust `head`/`tail`/`len` and `prev`/`next`, which are exactly what a
/// broken mutation path may have corrupted, so traversing the IR could loop
/// forever. Instead every arena is walked by raw id, which is bounded, shows
/// tombstones, and prints the same ids that the event log names.
pub(super) fn render<L: Dialect>(stage: &StageInfo<L>, events: &[MutationEvent]) -> String {
    let mut report = String::new();
    statements(&mut report, stage);
    blocks(&mut report, stage);
    ssa_values(&mut report, stage);
    mutation_events(&mut report, events);
    report
}

fn statements<L: Dialect>(report: &mut String, stage: &StageInfo<L>) {
    let items = &stage.nodes.statements.items;
    let _ = writeln!(report, "== statements ({} ids) ==", items.len());
    let _ = writeln!(
        report,
        "{:>5}  {:^3}  {:<12}  definition",
        "id", "del", "parent"
    );
    for (id, item) in items.iter().enumerate() {
        let _ = writeln!(
            report,
            "{:>5}  {:^3}  {:<12}  {:?}",
            id,
            tombstone(item.deleted()),
            option_debug(item.parent.as_ref()),
            item.definition,
        );
    }
}

fn blocks<L: Dialect>(report: &mut String, stage: &StageInfo<L>) {
    let items = &stage.nodes.blocks.items;
    let _ = writeln!(report, "\n== blocks ({} ids) ==", items.len());
    let _ = writeln!(
        report,
        "{:>5}  {:^3}  {:<10}  {:<16}  {:<40}  terminator",
        "id", "del", "parent", "predecessors", "stmts h/t/len"
    );
    for (id, item) in items.iter().enumerate() {
        let predecessors: Vec<String> = item.predecessors.iter().map(|b| b.to_string()).collect();
        let statements = format!(
            "{}/{}/{}",
            option_debug(item.statements.head.as_ref()),
            option_debug(item.statements.tail.as_ref()),
            item.statements.len,
        );
        let _ = writeln!(
            report,
            "{:>5}  {:^3}  {:<10}  {:<16}  {:<40}  {}",
            id,
            tombstone(item.deleted()),
            option_debug(item.parent.as_ref()),
            format!("[{}]", predecessors.join(", ")),
            statements,
            option_debug(item.terminator.as_ref()),
        );
    }
}

fn ssa_values<L: Dialect>(report: &mut String, stage: &StageInfo<L>) {
    let items = &stage.ssas.items;
    let _ = writeln!(report, "\n== ssa values ({} ids) ==", items.len());
    let _ = writeln!(report, "{:>5}  {:^3}  {:<30}  uses", "id", "del", "kind");
    for (id, item) in items.iter().enumerate() {
        let (kind, uses) = match &**item {
            Some(info) => (format!("{:?}", info.kind), format!("{:?}", info.uses)),
            None => ("-".to_string(), "-".to_string()),
        };
        let _ = writeln!(
            report,
            "{:>5}  {:^3}  {:<30}  {}",
            id,
            tombstone(item.deleted()),
            kind,
            uses,
        );
    }
}

fn mutation_events(report: &mut String, events: &[MutationEvent]) {
    let _ = writeln!(report, "\n== mutation events ({}) ==", events.len());
    for (index, event) in events.iter().enumerate() {
        let _ = writeln!(report, "{index:>5}  {event:?}");
    }
}

fn tombstone(deleted: bool) -> &'static str {
    if deleted { "x" } else { "" }
}

fn option_debug<T: fmt::Debug>(value: Option<&T>) -> String {
    match value {
        Some(value) => format!("{value:?}"),
        None => "-".to_string(),
    }
}
