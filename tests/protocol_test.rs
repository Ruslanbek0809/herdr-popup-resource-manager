mod common;

use herdr_popup_resource_manager::herdr::protocol::{Snapshot, Workspace};

#[test]
fn snapshot_fixture_deserializes_in_herdr_order() {
    let snap = common::snapshot();
    let ws: Vec<&str> = snap
        .workspaces
        .iter()
        .map(|w| w.workspace_id.as_str())
        .collect();
    assert_eq!(ws, ["w8", "w9"]);
    let tabs: Vec<&str> = snap.tabs.iter().map(|t| t.tab_id.as_str()).collect();
    assert_eq!(tabs, ["w8:t1", "w8:tC", "w9:t1", "w9:t5", "w9:t7"]);
    assert_eq!(snap.panes.len(), 5);
}

#[test]
fn optional_pane_fields_are_optional() {
    let snap = common::snapshot();
    let agent = snap.panes.iter().find(|p| p.pane_id == "w8:p1").unwrap();
    assert_eq!(agent.agent.as_deref(), Some("claude"));
    assert_eq!(agent.agent_status.as_deref(), Some("idle"));
    assert_eq!(
        agent.terminal_title_stripped.as_deref(),
        Some("Review commits and clean up")
    );
    let files = snap.panes.iter().find(|p| p.pane_id == "w9:pD").unwrap();
    assert_eq!(files.agent, None);
    assert_eq!(files.label.as_deref(), Some("Files"));
    assert!(snap.workspaces.iter().all(|w| w.worktree.is_none()));
}

#[test]
fn worktree_info_deserializes_when_present() {
    let ws: Workspace = serde_json::from_str(
        r#"{"workspace_id":"w3","label":"x","worktree":{"repo_key":"r1","repo_name":"r","repo_root":"/r",
            "checkout_path":"/r2","is_linked_worktree":true}}"#,
    )
    .unwrap();
    let wt = ws.worktree.unwrap();
    assert_eq!((wt.repo_key.as_str(), wt.is_linked_worktree), ("r1", true));
}

#[test]
fn unknown_fields_are_ignored() {
    let snap: Snapshot = serde_json::from_str(
        r#"{"workspaces":[],"tabs":[],"panes":[],"layouts":{},"new_field":1}"#,
    )
    .unwrap();
    assert!(snap.panes.is_empty());
}

#[test]
fn process_info_fixture_deserializes() {
    let infos = common::infos();
    let info = infos["w8:p1"].as_ref().unwrap();
    assert_eq!(
        (info.shell_pid, info.foreground_process_group_id),
        (Some(11959), Some(12032))
    );
    let claude = info
        .foreground_processes
        .iter()
        .find(|p| p.pid == 12032)
        .unwrap();
    assert_eq!(claude.argv[0], "claude");
    assert_eq!(claude.name.as_deref(), Some("2.1.299"));
}
