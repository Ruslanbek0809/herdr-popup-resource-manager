//! Shared test fixtures: real captures from macOS + herdr 0.9.3, anonymised.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

use herdr_popup_resource_manager::herdr::protocol::{ProcessInfo, Snapshot};
use herdr_popup_resource_manager::procs::{ProcTable, parse_ps};

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).expect("fixture exists")
}

pub fn snapshot() -> Snapshot {
    serde_json::from_str(&fixture("snapshot.json")).expect("snapshot fixture parses")
}

pub fn infos() -> HashMap<String, Option<ProcessInfo>> {
    serde_json::from_str(&fixture("process_info.json")).expect("process_info fixture parses")
}

pub fn procs() -> ProcTable {
    parse_ps(&fixture("ps.txt"))
}

/// A pid that isn't in the fixture, so no pane is excluded as "our own".
pub const NO_PID: u32 = 999_999;
