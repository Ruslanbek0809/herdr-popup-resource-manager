mod common;

use std::collections::HashMap;
use std::time::Duration;

use herdr_popup_resource_manager::procs::{
    Proc, ProcTable, Usage, app_name, children_map, parse_cpu_time, parse_ps, subtree, usage_of,
};

#[test]
fn cpu_time_formats_from_ps() {
    assert_eq!(parse_cpu_time("0:02.93"), Some(2_930));
    assert_eq!(
        parse_cpu_time("3133:45.79"),
        Some((3133 * 60 + 45) * 1000 + 790)
    );
    assert_eq!(
        parse_cpu_time("02:03:04"),
        Some((2 * 3600 + 3 * 60 + 4) * 1000)
    );
    assert_eq!(
        parse_cpu_time("1-02:03:04"),
        Some((86_400 + 2 * 3600 + 3 * 60 + 4) * 1000)
    );
    assert_eq!(parse_cpu_time("garbage"), None);
}

#[test]
fn ps_fixture_parses_every_row() {
    let procs = common::procs();
    assert_eq!(procs.len(), 62);
    let claude = &procs[&12032];
    assert_eq!(
        (claude.ppid, claude.rss, claude.comm.as_str()),
        (11959, 255_424 * 1024, "claude")
    );
    assert_eq!(claude.cpu_ms, 32_390);
}

#[test]
fn comm_with_spaces_is_kept_whole() {
    let procs = parse_ps(
        "  10     1   0:01.00   2048 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome\n",
    );
    assert_eq!(
        procs[&10].comm,
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    );
}

#[test]
fn app_name_groups_helpers_under_their_app() {
    assert_eq!(
        app_name("/Applications/Google Chrome.app/Contents/Frameworks/x/Helper (Renderer)"),
        "Google Chrome"
    );
    assert_eq!(app_name("/usr/sbin/systemstats"), "systemstats");
    assert_eq!(
        app_name("kworker/0:1"),
        "kworker/0:1".rsplit('/').next().unwrap()
    );
}

#[test]
fn subtree_walks_children() {
    let procs = common::procs();
    let kids = children_map(&procs);
    let mut tree = subtree(11959, &kids);
    tree.sort();
    assert_eq!(tree, vec![11959, 12032, 13287, 13348]);
}

fn table(entries: &[(u32, u64)]) -> ProcTable {
    entries
        .iter()
        .map(|&(pid, cpu_ms)| {
            (
                pid,
                Proc {
                    pid,
                    ppid: 1,
                    cpu_ms,
                    rss: 1024,
                    comm: "x".into(),
                },
            )
        })
        .collect()
}

#[test]
fn first_sample_has_no_cpu() {
    let usage = usage_of(&[2], &table(&[(2, 5_000)]), None, None);
    assert_eq!(usage.cpu, None);
    assert_eq!(usage.rss, 1024);
}

#[test]
fn cpu_is_a_per_pid_delta_so_an_exited_child_cannot_cancel_siblings() {
    let prev = HashMap::from([(2, 1_000), (3, 50_000)]); // pid 3 used 50 s and has exited since
    let usage = usage_of(
        &[2, 3],
        &table(&[(2, 1_500)]),
        Some(&prev),
        Some(Duration::from_secs(1)),
    );
    assert!((usage.cpu.unwrap() - 50.0).abs() < 1e-9);
}

#[test]
fn a_new_pid_counts_from_zero() {
    let usage = usage_of(
        &[4],
        &table(&[(4, 200)]),
        Some(&HashMap::new()),
        Some(Duration::from_secs(1)),
    );
    assert!((usage.cpu.unwrap() - 20.0).abs() < 1e-9);
}

#[test]
fn a_reused_pid_never_goes_negative() {
    let prev = HashMap::from([(2, 9_000)]);
    let usage = usage_of(
        &[2],
        &table(&[(2, 100)]),
        Some(&prev),
        Some(Duration::from_secs(1)),
    );
    assert_eq!(usage.cpu, Some(0.0));
}

#[test]
fn usage_sum_keeps_unmeasured_as_none() {
    assert_eq!(
        (Usage { cpu: None, rss: 1 } + Usage { cpu: None, rss: 2 }).cpu,
        None
    );
    assert_eq!(
        Usage { cpu: None, rss: 1 }
            + Usage {
                cpu: Some(3.0),
                rss: 2
            },
        Usage {
            cpu: Some(3.0),
            rss: 3
        }
    );
}
