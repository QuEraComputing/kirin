//! Shared builder helpers for directed and undirected graph construction.
//!
//! Both [`DiGraphBuilder`](super::digraph::DiGraphBuilder) and
//! [`UnGraphBuilder`](super::ungraph::UnGraphBuilder) perform identical
//! port allocation, name-to-index mapping, and placeholder resolution.
//! This module extracts that logic into reusable functions.

use std::collections::HashMap;

use crate::node::port::{Port, PortParent};
use crate::node::ssa::{BuilderSSAInfo, BuilderSSAKind, ResolutionInfo, SSAValue};
use crate::node::stmt::Statement;
use crate::{BuilderStageInfo, Dialect, Symbol};

/// The result of allocating ports and building name lookup maps.
pub(crate) struct AllocatedPorts {
    /// All ports: edge ports followed by capture ports.
    pub all_ports: Vec<Port>,
    /// The number of edge ports (the first `edge_count` elements).
    pub edge_count: usize,
    /// Mapping from port name symbol to positional index within edge ports.
    pub port_name_to_index: HashMap<Symbol, usize>,
    /// Mapping from capture name symbol to positional index within capture ports.
    pub capture_name_to_index: HashMap<Symbol, usize>,
}

/// Allocate ports and captures in the SSA arena, returning the allocated ports
/// and name-to-index lookup maps.
pub(crate) fn allocate_ports<L: Dialect>(
    stage: &mut BuilderStageInfo<L>,
    ports: Vec<(L::Type, Option<String>)>,
    captures: Vec<(L::Type, Option<String>)>,
    parent: PortParent,
) -> AllocatedPorts {
    let edge_count = ports.len();
    let mut all_ports = Vec::with_capacity(ports.len() + captures.len());

    for (index, (ty, name)) in ports.into_iter().enumerate() {
        let port: Port = stage.ssas.next_id().into();
        let ssa = BuilderSSAInfo::new(
            port.into(),
            name.map(|n| stage.symbols.intern(n)),
            Some(ty),
            BuilderSSAKind::Port(parent, index),
        );
        stage.ssas.alloc(ssa);
        all_ports.push(port);
    }

    for (i, (ty, name)) in captures.into_iter().enumerate() {
        let index = edge_count + i;
        let port: Port = stage.ssas.next_id().into();
        let ssa = BuilderSSAInfo::new(
            port.into(),
            name.map(|n| stage.symbols.intern(n)),
            Some(ty),
            BuilderSSAKind::Port(parent, index),
        );
        stage.ssas.alloc(ssa);
        all_ports.push(port);
    }

    let port_name_to_index: HashMap<Symbol, usize> = all_ports[..edge_count]
        .iter()
        .enumerate()
        .filter_map(|(i, port)| {
            let info = stage.ssas.get(SSAValue::from(*port))?;
            info.name().map(|sym| (sym, i))
        })
        .collect();

    let capture_name_to_index: HashMap<Symbol, usize> = all_ports[edge_count..]
        .iter()
        .enumerate()
        .filter_map(|(i, port)| {
            let info = stage.ssas.get(SSAValue::from(*port))?;
            info.name().map(|sym| (sym, i))
        })
        .collect();

    AllocatedPorts {
        all_ports,
        edge_count,
        port_name_to_index,
        capture_name_to_index,
    }
}

/// Resolve placeholder SSA values in the given statements and apply replacements.
///
/// Scans all arguments of `stmts` for unresolved port/capture references,
/// maps them to the corresponding allocated ports, then rewrites all
/// references and deletes the placeholder SSA values.
pub(crate) fn resolve_and_replace<L: Dialect>(
    stage: &mut BuilderStageInfo<L>,
    stmts: &[Statement],
    allocated: &AllocatedPorts,
    context: &str,
) {
    let mut replacements: HashMap<SSAValue, SSAValue> = HashMap::new();
    for &stmt_id in stmts {
        let info = &stage.statements[stmt_id];
        for arg in info.definition.arguments() {
            if replacements.contains_key(arg) {
                continue;
            }
            let ssa_info = stage.ssas.get(*arg).expect("SSAValue not found in stage");
            match ssa_info.kind {
                BuilderSSAKind::Unresolved(ResolutionInfo::Port(key)) => {
                    let index = super::resolve_builder_key(
                        key,
                        allocated.edge_count,
                        &allocated.port_name_to_index,
                        &stage.symbols,
                        &format!("{context} port"),
                    );
                    replacements.insert(*arg, allocated.all_ports[index].into());
                }
                BuilderSSAKind::Unresolved(ResolutionInfo::Capture(key)) => {
                    let index = super::resolve_builder_key(
                        key,
                        allocated.all_ports.len() - allocated.edge_count,
                        &allocated.capture_name_to_index,
                        &stage.symbols,
                        &format!("{context} capture"),
                    );
                    replacements.insert(
                        *arg,
                        allocated.all_ports[allocated.edge_count + index].into(),
                    );
                }
                _ => {}
            }
        }
    }

    // Delete placeholder SSAs
    for &old in replacements.keys() {
        stage.ssas.delete(old);
    }

    // Apply replacements
    for &stmt_id in stmts {
        let info = &mut stage.statements[stmt_id];
        for arg in info.definition.arguments_mut() {
            if let Some(&replacement) = replacements.get(arg) {
                *arg = replacement;
            }
        }
    }
}
