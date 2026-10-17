use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use herdr_popup_resource_manager::herdr::client::{Error, Herdr, SocketClient, Target};
use serde_json::{Value, json};

/// A fake herdr server: answers each connection with the next canned reply and records the request.
fn fake_herdr(replies: Vec<Value>) -> (PathBuf, Arc<Mutex<Vec<Value>>>) {
    let dir = std::env::temp_dir().join(format!("hprm-{}-{}", std::process::id(), rand_suffix()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("herdr.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_in = Arc::clone(&seen);
    thread::spawn(move || {
        for reply in replies {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            seen_in
                .lock()
                .unwrap()
                .push(serde_json::from_str::<Value>(&line).unwrap());
            (&stream)
                .write_all(format!("{reply}\n").as_bytes())
                .unwrap();
        }
    });
    (path, seen)
}

/// Unique per fake server: tests run in parallel and the clock is too coarse to tell them apart.
fn rand_suffix() -> usize {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn snapshot_sends_one_json_line_and_reads_the_result() {
    let (path, seen) = fake_herdr(vec![
        json!({"id": "x", "result": {"type": "session_snapshot",
        "snapshot": {"workspaces": [], "tabs": [], "panes": [{"pane_id": "w1:p1", "workspace_id": "w1", "tab_id": "w1:t1"}]}}}),
    ]);
    let snap = SocketClient::new(path).snapshot().unwrap();
    assert_eq!(snap.panes[0].pane_id, "w1:p1");
    let req = &seen.lock().unwrap()[0];
    assert_eq!(req["method"], "session.snapshot");
    assert_eq!(req["params"], json!({}));
    assert!(req["id"].is_string());
}

#[test]
fn process_info_passes_the_pane_id() {
    let (path, seen) = fake_herdr(vec![json!({"id": "x", "result": {"process_info":
        {"pane_id": "w1:p1", "shell_pid": 42, "foreground_process_group_id": 42, "foreground_processes": []}}})]);
    let info = SocketClient::new(path).process_info("w1:p1").unwrap();
    assert_eq!(info.shell_pid, Some(42));
    assert_eq!(
        seen.lock().unwrap()[0]["params"],
        json!({"pane_id": "w1:p1"})
    );
}

#[test]
fn herdr_errors_come_back_as_errors() {
    let (path, _) = fake_herdr(vec![
        json!({"id": "x", "error": {"code": "pane_not_found", "message": "pane not found"}}),
    ]);
    match SocketClient::new(path).process_info("w9:p9") {
        Err(Error::Herdr { code, message, .. }) => assert_eq!(
            (code.as_str(), message.as_str()),
            ("pane_not_found", "pane not found")
        ),
        other => panic!("expected a herdr error, got {other:?}"),
    }
}

#[test]
fn focus_and_close_use_the_matching_methods() {
    let ok = json!({"id": "x", "result": {"type": "ok"}});
    let (path, seen) = fake_herdr(vec![ok.clone(), ok.clone(), ok.clone(), ok]);
    let mut client = SocketClient::new(path);
    client.focus(&Target::Pane("w1:p1".into())).unwrap();
    client.focus(&Target::Tab("w1:t1".into())).unwrap();
    client.focus(&Target::Workspace("w1".into())).unwrap();
    client.close_pane("w1:p2").unwrap();
    let calls: Vec<(String, Value)> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["method"].as_str().unwrap().to_string(),
                r["params"].clone(),
            )
        })
        .collect();
    assert_eq!(
        calls,
        vec![
            ("pane.focus".to_string(), json!({"pane_id": "w1:p1"})),
            ("tab.focus".to_string(), json!({"tab_id": "w1:t1"})),
            ("workspace.focus".to_string(), json!({"workspace_id": "w1"})),
            ("pane.close".to_string(), json!({"pane_id": "w1:p2"})),
        ]
    );
}

#[test]
fn a_missing_socket_is_an_io_error() {
    let err = SocketClient::new(PathBuf::from("/nonexistent/herdr.sock"))
        .snapshot()
        .unwrap_err();
    assert!(matches!(err, Error::Io(_)), "{err:?}");
    assert!(err.to_string().contains("/nonexistent/herdr.sock"));
}

/// Against the real herdr (read-only). Run inside herdr: `cargo test -- --ignored live_`.
#[test]
#[ignore]
fn live_snapshot_and_process_info_deserialize() {
    let mut client = SocketClient::from_env();
    let snap = client.snapshot().expect("live snapshot");
    assert!(!snap.panes.is_empty());
    for pane in &snap.panes {
        let info = client
            .process_info(&pane.pane_id)
            .expect("live process_info");
        assert!(info.shell_pid.is_some(), "{} has a shell", pane.pane_id);
    }
    println!(
        "live: {} workspaces, {} tabs, {} panes",
        snap.workspaces.len(),
        snap.tabs.len(),
        snap.panes.len()
    );
}
