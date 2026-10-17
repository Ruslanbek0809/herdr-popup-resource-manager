mod common;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use common::NO_PID;
use herdr_popup_resource_manager::herdr::protocol::Workspace;
use herdr_popup_resource_manager::procs::Usage;
use herdr_popup_resource_manager::tree::{Node, NodeKind, build_tree, flatten, order_workspaces};

fn build(
    own_pid: u32,
    prev: Option<&HashMap<u32, u64>>,
    dt: Option<Duration>,
) -> (Vec<Node>, Usage) {
    build_tree(
        &common::snapshot(),
        &common::infos(),
        &common::procs(),
        prev,
        dt,
        own_pid,
    )
}

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    nodes.iter().find_map(|n| {
        if n.id == id {
            Some(n)
        } else {
            find(&n.children, id)
        }
    })
}

#[test]
fn shape_follows_snapshot_order() {
    let (nodes, _) = build(NO_PID, None, None);
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["w8", "w9", "outside"]);
    let tabs: Vec<&str> = nodes[1].children.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(tabs, ["w9:t1", "w9:t5", "w9:t7"]);
    assert_eq!(nodes[0].kind, NodeKind::Workspace);
    assert_eq!(nodes[0].children[0].kind, NodeKind::Tab);
    assert_eq!(nodes[0].children[0].children[0].kind, NodeKind::Pane);
}

#[test]
fn a_pane_sums_its_whole_process_tree() {
    let (nodes, _) = build(NO_PID, None, None);
    // shell 11959 → claude 12032 → dart 13287 → dart 13348
    assert_eq!(
        find(&nodes, "w8:p1").unwrap().usage.rss,
        (16 + 255_424 + 3_952 + 40_080) * 1024
    );
}

#[test]
fn parents_sum_children_and_the_total_excludes_outside() {
    let (nodes, total) = build(NO_PID, None, None);
    let ws = find(&nodes, "w9").unwrap();
    assert_eq!(
        ws.usage.rss,
        ws.children.iter().map(|t| t.usage.rss).sum::<u64>()
    );
    assert_eq!(
        total.rss,
        find(&nodes, "w8").unwrap().usage.rss + ws.usage.rss
    );
}

#[test]
fn pane_names_and_details() {
    let (nodes, _) = build(NO_PID, None, None);
    let p1 = find(&nodes, "w8:p1").unwrap();
    assert_eq!(
        (p1.label.as_str(), p1.detail.as_str(), p1.status.as_deref()),
        ("claude", "Review commits and clean up", Some("idle"))
    );
    let shell = find(&nodes, "w9:pD").unwrap();
    assert_eq!(
        (
            shell.label.as_str(),
            shell.detail.as_str(),
            shell.status.as_deref()
        ),
        ("shell", "zsh", None)
    );
}

#[test]
fn the_pane_running_this_process_is_left_out() {
    let (nodes, _) = build(46311, None, None); // a zsh under claude 34372 in pane w8:pN
    assert!(find(&nodes, "w8:pN").is_none());
    assert!(
        find(&nodes, "w8:tC").is_none(),
        "a tab without panes disappears"
    );
    assert!(find(&nodes, "w8:p1").is_some());
}

#[test]
fn outside_has_every_unclaimed_process_and_the_top_three_apps() {
    let (nodes, total) = build(NO_PID, None, None);
    let outside = find(&nodes, "outside").unwrap();
    assert_eq!(outside.kind, NodeKind::Outside);
    assert_eq!(
        total.rss + outside.usage.rss,
        common::procs().values().map(|p| p.rss).sum::<u64>()
    );
    assert_eq!(outside.children.len(), 3);
    assert!(outside.children.iter().all(|c| c.kind == NodeKind::App));
    // Chrome's helpers are grouped under the app, so it is the biggest app by memory with no CPU yet.
    assert_eq!(outside.children[0].label, "Google Chrome");
}

#[test]
fn cpu_appears_after_a_second_sample() {
    let mut prev: HashMap<u32, u64> = common::procs()
        .values()
        .map(|p| (p.pid, p.cpu_ms))
        .collect();
    *prev.get_mut(&12032).unwrap() -= 500; // claude in w8:p1 used half a core over 1 s
    let (nodes, _) = build(NO_PID, Some(&prev), Some(Duration::from_secs(1)));
    assert!((find(&nodes, "w8:p1").unwrap().usage.cpu.unwrap() - 50.0).abs() < 1e-9);
    assert_eq!(find(&nodes, "w9:p1").unwrap().usage.cpu, Some(0.0));
}

#[test]
fn a_pane_without_process_info_still_shows() {
    let mut infos = common::infos();
    infos.insert("w9:p1".into(), None);
    let (nodes, _) = build_tree(
        &common::snapshot(),
        &infos,
        &common::procs(),
        None,
        None,
        NO_PID,
    );
    let pane = find(&nodes, "w9:p1").unwrap();
    assert_eq!(
        (pane.label.as_str(), pane.usage.rss, pane.usage.cpu),
        ("claude", 0, None)
    );
}

fn ws(id: &str, repo: Option<(&str, bool)>) -> Workspace {
    let worktree = repo.map(|(key, linked)| {
        format!(r#","worktree":{{"repo_key":"{key}","is_linked_worktree":{linked}}}"#)
    });
    serde_json::from_str(&format!(
        r#"{{"workspace_id":"{id}","label":"{id}"{}}}"#,
        worktree.unwrap_or_default()
    ))
    .unwrap()
}

#[test]
fn linked_worktrees_follow_their_primary() {
    let list = [
        ws("a", Some(("r1", true))),
        ws("b", None),
        ws("c", Some(("r1", false))),
        ws("d", Some(("r2", true))),
    ];
    let order: Vec<(&str, bool)> = order_workspaces(&list)
        .into_iter()
        .map(|(w, linked)| (w.workspace_id.as_str(), linked))
        .collect();
    assert_eq!(
        order,
        [("b", false), ("c", false), ("a", true), ("d", true)]
    );
}

#[test]
fn linked_worktree_rows_are_marked() {
    let mut snap = common::snapshot();
    snap.workspaces[0] = ws("w8", Some(("r1", false)));
    snap.workspaces[1] = ws("w9", Some(("r1", true)));
    let (nodes, _) = build_tree(
        &snap,
        &common::infos(),
        &common::procs(),
        None,
        None,
        NO_PID,
    );
    assert_eq!(nodes[1].label, "↳ w9");
}

#[test]
fn flatten_respects_folds() {
    let (nodes, _) = build(NO_PID, None, None);
    let all = flatten(&nodes, &HashSet::new());
    let folded = flatten(&nodes, &HashSet::from(["w9".to_string()]));
    let w9 = find(&nodes, "w9").unwrap();
    let hidden = w9.children.len() + w9.children.iter().map(|t| t.children.len()).sum::<usize>();
    assert_eq!(all.len() - folded.len(), hidden);
    let depths: Vec<usize> = all.iter().take(3).map(|r| r.depth).collect();
    assert_eq!(depths, [0, 1, 2]);
    assert!(all[0].foldable && !all[2].foldable);
}
