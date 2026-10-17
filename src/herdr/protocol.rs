//! Typed views of the parts of herdr's socket API this plugin reads. Unknown fields are ignored.

use serde::Deserialize;

/// `session.snapshot` → `result.snapshot`. Arrays are in herdr's display order.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub tabs: Vec<Tab>,
    #[serde(default)]
    pub panes: Vec<Pane>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Workspace {
    pub workspace_id: String,
    pub label: Option<String>,
    pub worktree: Option<Worktree>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Worktree {
    pub repo_key: String,
    pub is_linked_worktree: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tab {
    pub tab_id: String,
    pub workspace_id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pane {
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    /// Absent on panes without a recognised agent.
    pub agent: Option<String>,
    /// idle | working | blocked | done | unknown
    pub agent_status: Option<String>,
    pub terminal_title_stripped: Option<String>,
    pub label: Option<String>,
}

/// `pane.process_info` → `result.process_info`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProcessInfo {
    pub shell_pid: Option<u32>,
    pub foreground_process_group_id: Option<u32>,
    #[serde(default)]
    pub foreground_processes: Vec<ForegroundProcess>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForegroundProcess {
    pub pid: u32,
    /// For claude this is its version (`2.1.299`): prefer `argv[0]`.
    pub name: Option<String>,
    pub argv0: Option<String>,
    #[serde(default)]
    pub argv: Vec<String>,
}
