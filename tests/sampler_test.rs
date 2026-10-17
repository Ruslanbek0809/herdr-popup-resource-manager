mod common;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::NO_PID;
use herdr_popup_resource_manager::herdr::client::{Error, Herdr, Target};
use herdr_popup_resource_manager::herdr::protocol::{ProcessInfo, Snapshot};
use herdr_popup_resource_manager::procs::{ProcSource, ProcTable, PsSource, SysinfoSource};
use herdr_popup_resource_manager::sampler::Sampler;
use herdr_popup_resource_manager::tree::Node;

/// herdr from the fixtures; `w9:p5` vanishes between the snapshot and its process_info call.
struct FakeHerdr;

impl Herdr for FakeHerdr {
    fn snapshot(&mut self) -> Result<Snapshot, Error> {
        Ok(common::snapshot())
    }
    fn process_info(&mut self, pane_id: &str) -> Result<ProcessInfo, Error> {
        if pane_id == "w9:p5" {
            return Err(Error::Herdr {
                method: "pane.process_info".into(),
                code: "pane_not_found".into(),
                message: "pane not found".into(),
            });
        }
        Ok(common::infos()[pane_id].clone().unwrap())
    }
    fn focus(&mut self, _: &Target) -> Result<(), Error> {
        unreachable!("the sampler never focuses")
    }
    fn close_pane(&mut self, _: &str) -> Result<(), Error> {
        unreachable!("the sampler never closes")
    }
}

/// The fixture table; claude 12174 (pane w9:p1) burns two cores for every second of fake time.
struct BusyProcs {
    calls: u64,
}

impl ProcSource for BusyProcs {
    fn read(&mut self) -> io::Result<ProcTable> {
        let mut procs = common::procs();
        procs.get_mut(&12174).unwrap().cpu_ms += self.calls * 2_000;
        self.calls += 1;
        Ok(procs)
    }
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
fn cpu_needs_two_samples_and_follows_elapsed_time() {
    let mut sampler = Sampler::new(FakeHerdr, BusyProcs { calls: 0 }, NO_PID);
    let t0 = Instant::now();
    let first = sampler.sample(|| t0).unwrap();
    assert_eq!(find(&first.nodes, "w9:p1").unwrap().usage.cpu, None);
    let second = sampler.sample(|| t0 + Duration::from_secs(1)).unwrap();
    assert!((find(&second.nodes, "w9:p1").unwrap().usage.cpu.unwrap() - 200.0).abs() < 1e-9);
}

#[test]
fn a_pane_that_vanishes_mid_sample_shows_without_numbers() {
    let mut sampler = Sampler::new(FakeHerdr, BusyProcs { calls: 0 }, NO_PID);
    let frame = sampler.sample(Instant::now).unwrap();
    assert_eq!(find(&frame.nodes, "w9:p5").unwrap().usage.rss, 0);
}

#[test]
fn a_failed_snapshot_is_an_error() {
    struct Down;
    impl Herdr for Down {
        fn snapshot(&mut self) -> Result<Snapshot, Error> {
            Err(Error::Io("herdr socket /x: connection refused".into()))
        }
        fn process_info(&mut self, _: &str) -> Result<ProcessInfo, Error> {
            unreachable!()
        }
        fn focus(&mut self, _: &Target) -> Result<(), Error> {
            unreachable!()
        }
        fn close_pane(&mut self, _: &str) -> Result<(), Error> {
            unreachable!()
        }
    }
    let err = Sampler::new(Down, BusyProcs { calls: 0 }, NO_PID)
        .sample(Instant::now)
        .unwrap_err();
    assert!(err.to_string().contains("connection refused"));
}

#[test]
fn ps_source_looks_names_up_once_per_new_pid() {
    let tables = Arc::new(Mutex::new(vec![
        "  1  0  0:01.10  100\n  3  1  0:00.00  300\n".to_string(),
        "  1  0  0:01.00  100\n  2  1  0:00.50  200\n".to_string(),
    ]));
    let name_calls = Arc::new(Mutex::new(Vec::new()));
    let (t, n) = (Arc::clone(&tables), Arc::clone(&name_calls));
    let mut source = PsSource::with(
        move |args: &[&str]| {
            assert_eq!(
                args,
                ["-A", "-o", "pid=,ppid=,time=,rss="],
                "one ps call per sample"
            );
            Ok(t.lock().unwrap().pop().unwrap())
        },
        move |pids: &[u32]| {
            let mut pids = pids.to_vec();
            pids.sort();
            n.lock().unwrap().push(pids.clone());
            let known = |p: u32| match p {
                1 => "/sbin/launchd",
                2 => "/usr/bin/yes",
                _ => "/Applications/A B.app/Contents/MacOS/A B",
            };
            pids.into_iter()
                .map(|p| (p, known(p).to_string()))
                .collect()
        },
    );
    let first = source.read().unwrap();
    assert_eq!(
        (first[&2].comm.as_str(), first[&2].rss),
        ("/usr/bin/yes", 200 * 1024)
    );
    let second = source.read().unwrap();
    assert_eq!(*name_calls.lock().unwrap(), [vec![1, 2], vec![3]]);
    assert_eq!(second[&3].comm, "/Applications/A B.app/Contents/MacOS/A B");
    assert!(!second.contains_key(&2), "exited pids are gone");
}

#[cfg(target_os = "macos")]
#[test]
fn ps_source_names_come_from_the_system_for_root_processes_too() {
    let mut source = PsSource::new();
    let procs = source.read().unwrap();
    let launchd = &procs[&1];
    assert!(
        launchd.comm.ends_with("launchd"),
        "pid 1 is named: {:?}",
        launchd.comm
    );
    assert!(launchd.rss > 0, "ps reads root-owned memory");
}

#[test]
fn sysinfo_source_sees_this_process() {
    let procs = SysinfoSource::new().read().unwrap();
    let me = &procs[&std::process::id()];
    assert!(me.rss > 0);
    assert!(procs.contains_key(&me.ppid), "parent {} is listed", me.ppid);
}

/// Fake time shared by a herdr that answers slowly and a process table that reads the clock.
type Clock = Arc<Mutex<Instant>>;

/// herdr from the fixtures; each sample's snapshot takes the next delay from `delays`.
struct SlowHerdr {
    clock: Clock,
    delays: Vec<Duration>,
}

impl Herdr for SlowHerdr {
    fn snapshot(&mut self) -> Result<Snapshot, Error> {
        *self.clock.lock().unwrap() += self.delays.remove(0);
        FakeHerdr.snapshot()
    }
    fn process_info(&mut self, pane_id: &str) -> Result<ProcessInfo, Error> {
        FakeHerdr.process_info(pane_id)
    }
    fn focus(&mut self, _: &Target) -> Result<(), Error> {
        unreachable!()
    }
    fn close_pane(&mut self, _: &str) -> Result<(), Error> {
        unreachable!()
    }
}

/// claude 12174 burns two cores of the fake time that passed before each read.
struct TimedProcs {
    clock: Clock,
    start: Instant,
}

impl ProcSource for TimedProcs {
    fn read(&mut self) -> io::Result<ProcTable> {
        let elapsed = self.clock.lock().unwrap().duration_since(self.start);
        let mut procs = common::procs();
        procs.get_mut(&12174).unwrap().cpu_ms += elapsed.as_millis() as u64 * 2;
        Ok(procs)
    }
}

#[test]
fn cpu_is_timed_from_the_process_reads_so_a_slow_herdr_doesnt_skew_it() {
    let start = Instant::now();
    let clock: Clock = Arc::new(Mutex::new(start));
    let herdr = SlowHerdr {
        clock: Arc::clone(&clock),
        delays: vec![Duration::from_millis(10), Duration::from_millis(300)],
    };
    let procs = TimedProcs {
        clock: Arc::clone(&clock),
        start,
    };
    let mut sampler = Sampler::new(herdr, procs, NO_PID);
    let now = || *clock.lock().unwrap();
    sampler.sample(now).unwrap();
    *clock.lock().unwrap() += Duration::from_secs(1); // the gap between samples
    let frame = sampler.sample(now).unwrap();
    let cpu = find(&frame.nodes, "w9:p1").unwrap().usage.cpu.unwrap();
    assert!((cpu - 200.0).abs() < 1e-6, "cpu {cpu}");
}
