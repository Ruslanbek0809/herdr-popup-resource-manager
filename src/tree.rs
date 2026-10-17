//! Workspace → tab → pane tree with usage sums, plus the "Outside herdr" row.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::Serialize;

use crate::herdr::protocol::{Pane, ProcessInfo, Snapshot, Workspace};
use crate::procs::{ProcTable, Usage, app_name, children_map, subtree, usage_of};

/// How many apps the "Outside herdr" row lists.
pub const OUTSIDE_TOP: usize = 3;
pub const OUTSIDE_ID: &str = "outside";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Workspace,
    Tab,
    Pane,
    Outside,
    App,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub detail: String,
    /// The agent's status (idle, working, blocked, done, unknown); `None` for panes without an agent.
    pub status: Option<String>,
    pub usage: Usage,
    pub children: Vec<Node>,
    pub workspace_id: String,
    pub tab_id: String,
    pub pane_id: String,
}

impl Node {
    fn new(id: &str, kind: NodeKind, label: String) -> Node {
        Node {
            id: id.to_string(),
            kind,
            label,
            detail: String::new(),
            status: None,
            usage: Usage::UNMEASURED,
            children: Vec::new(),
            workspace_id: String::new(),
            tab_id: String::new(),
            pane_id: String::new(),
        }
    }
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// (name, detail) for a pane row, e.g. ("claude", "watch #9972") or ("shell", "zsh").
pub fn pane_display(pane: &Pane, info: Option<&ProcessInfo>) -> (String, String) {
    let title = pane.terminal_title_stripped.clone().unwrap_or_default();
    let Some(info) = info else {
        let name = pane
            .agent
            .clone()
            .or_else(|| pane.label.clone())
            .unwrap_or_else(|| "pane".into());
        return (name, title);
    };
    let fg = &info.foreground_processes;
    let main = fg
        .iter()
        .find(|p| Some(p.pid) == info.foreground_process_group_id)
        .or(fg.last());
    let main = match main {
        Some(m) if Some(m.pid) != info.shell_pid => m,
        other => {
            let shell = other
                .and_then(|m| m.argv0.as_deref())
                .map(basename)
                .unwrap_or("");
            let shell = if shell.is_empty() { "sh" } else { shell };
            return ("shell".into(), shell.into());
        }
    };
    let argv0 = main
        .argv
        .first()
        .or(main.argv0.as_ref())
        .map_or("?", |a| a.as_str());
    let name = pane
        .agent
        .clone()
        .unwrap_or_else(|| basename(argv0).to_string());
    let detail = if title.is_empty() {
        main.argv
            .iter()
            .skip(1)
            .take(2)
            .map(|a| basename(a))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        title
    };
    (name, detail)
}

/// Snapshot order, with linked worktree workspaces right after their primary.
/// Returns (workspace, is_linked); a linked worktree without its primary open stays in place.
pub fn order_workspaces(workspaces: &[Workspace]) -> Vec<(&Workspace, bool)> {
    let is_linked = |w: &Workspace| w.worktree.as_ref().is_some_and(|wt| wt.is_linked_worktree);
    let mut primaries: HashMap<&str, &str> = HashMap::new();
    for w in workspaces {
        if let Some(wt) = w.worktree.as_ref().filter(|wt| !wt.is_linked_worktree) {
            primaries.entry(&wt.repo_key).or_insert(&w.workspace_id);
        }
    }
    let primary_of = |w: &Workspace| -> Option<&str> {
        let wt = w.worktree.as_ref().filter(|wt| wt.is_linked_worktree)?;
        primaries.get(wt.repo_key.as_str()).copied()
    };
    let mut out = Vec::new();
    for w in workspaces.iter().filter(|w| primary_of(w).is_none()) {
        out.push((w, is_linked(w)));
        out.extend(
            workspaces
                .iter()
                .filter(|l| primary_of(l) == Some(w.workspace_id.as_str()))
                .map(|l| (l, true)),
        );
    }
    out
}

/// Builds the tree from one sample. `own_pid`'s process tree (this popup) is left out everywhere.
/// Returns the top-level nodes (workspaces, then "Outside herdr") and the herdr total.
pub fn build_tree(
    snapshot: &Snapshot,
    infos: &HashMap<String, Option<ProcessInfo>>,
    procs: &ProcTable,
    prev_cpu: Option<&HashMap<u32, u64>>,
    dt: Option<Duration>,
    own_pid: u32,
) -> (Vec<Node>, Usage) {
    let kids = children_map(procs);
    let own: HashSet<u32> = subtree(own_pid, &kids).into_iter().collect();
    let mut claimed = own.clone();

    let mut panes: Vec<Node> = Vec::new();
    for pane in &snapshot.panes {
        let info = infos.get(&pane.pane_id).and_then(Option::as_ref);
        let pids = match info.and_then(|i| i.shell_pid) {
            Some(shell) if procs.contains_key(&shell) => subtree(shell, &kids),
            _ => Vec::new(),
        };
        if pids.iter().any(|p| own.contains(p)) {
            continue; // the pane hosting this process
        }
        let pids: Vec<u32> = pids.into_iter().filter(|p| claimed.insert(*p)).collect();
        let (label, detail) = pane_display(pane, info);
        let mut node = Node::new(&pane.pane_id, NodeKind::Pane, label);
        node.detail = detail;
        node.status = pane.agent.as_ref().and(pane.agent_status.clone());
        node.usage = if pids.is_empty() {
            Usage::UNMEASURED
        } else {
            usage_of(&pids, procs, prev_cpu, dt)
        };
        node.workspace_id = pane.workspace_id.clone();
        node.tab_id = pane.tab_id.clone();
        node.pane_id = pane.pane_id.clone();
        panes.push(node);
    }

    let mut tabs_by_ws: HashMap<&str, Vec<Node>> = HashMap::new();
    for tab in &snapshot.tabs {
        let children: Vec<Node> = panes
            .iter()
            .filter(|p| p.tab_id == tab.tab_id)
            .cloned()
            .collect();
        if children.is_empty() {
            continue;
        }
        let mut node = Node::new(
            &tab.tab_id,
            NodeKind::Tab,
            tab.label.clone().unwrap_or_else(|| tab.tab_id.clone()),
        );
        node.usage = children.iter().map(|p| p.usage).sum();
        node.children = children;
        node.workspace_id = tab.workspace_id.clone();
        node.tab_id = tab.tab_id.clone();
        tabs_by_ws.entry(&tab.workspace_id).or_default().push(node);
    }

    let mut nodes = Vec::new();
    let mut total = Usage::UNMEASURED;
    for (ws, linked) in order_workspaces(&snapshot.workspaces) {
        let Some(tabs) = tabs_by_ws.remove(ws.workspace_id.as_str()) else {
            continue;
        };
        let label = ws.label.clone().unwrap_or_else(|| ws.workspace_id.clone());
        let mut node = Node::new(
            &ws.workspace_id,
            NodeKind::Workspace,
            if linked {
                format!("↳ {label}")
            } else {
                label
            },
        );
        node.usage = tabs.iter().map(|t| t.usage).sum();
        node.children = tabs;
        node.workspace_id = ws.workspace_id.clone();
        total = total + node.usage;
        nodes.push(node);
    }

    nodes.push(outside(procs, &claimed, prev_cpu, dt));
    (nodes, total)
}

fn outside(
    procs: &ProcTable,
    claimed: &HashSet<u32>,
    prev_cpu: Option<&HashMap<u32, u64>>,
    dt: Option<Duration>,
) -> Node {
    let rest: Vec<u32> = procs
        .keys()
        .copied()
        .filter(|p| !claimed.contains(p))
        .collect();
    let mut by_app: HashMap<String, Vec<u32>> = HashMap::new();
    for pid in &rest {
        by_app
            .entry(app_name(&procs[pid].comm))
            .or_default()
            .push(*pid);
    }
    let mut apps: Vec<Node> = by_app
        .into_iter()
        .map(|(name, pids)| {
            let mut node = Node::new(&format!("app:{name}"), NodeKind::App, name);
            node.usage = usage_of(&pids, procs, prev_cpu, dt);
            node
        })
        .collect();
    apps.sort_by(|a, b| {
        let key = |n: &Node| (n.usage.cpu.unwrap_or(0.0), n.usage.rss);
        key(b)
            .partial_cmp(&key(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.label.cmp(&b.label))
    });
    apps.truncate(OUTSIDE_TOP);
    let mut node = Node::new(OUTSIDE_ID, NodeKind::Outside, "Outside herdr".into());
    node.detail = "other apps and daemons".into();
    node.usage = usage_of(&rest, procs, prev_cpu, dt);
    node.children = apps;
    node
}

/// One visible line of the tree.
#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    pub node: &'a Node,
    pub depth: usize,
    pub foldable: bool,
}

/// The visible rows, skipping the children of folded nodes.
pub fn flatten<'a>(nodes: &'a [Node], folded: &HashSet<String>) -> Vec<Row<'a>> {
    fn walk<'a>(nodes: &'a [Node], folded: &HashSet<String>, depth: usize, out: &mut Vec<Row<'a>>) {
        for node in nodes {
            out.push(Row {
                node,
                depth,
                foldable: !node.children.is_empty(),
            });
            if !folded.contains(&node.id) {
                walk(&node.children, folded, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, folded, 0, &mut out);
    out
}
