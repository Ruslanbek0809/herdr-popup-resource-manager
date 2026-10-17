//! The process table and per-pid CPU/memory accounting.

use std::collections::HashMap;
use std::ops::Add;
use std::time::Duration;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    /// Cumulative CPU time in milliseconds.
    pub cpu_ms: u64,
    /// Resident memory in bytes.
    pub rss: u64,
    pub comm: String,
}

pub type ProcTable = HashMap<u32, Proc>;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Usage {
    /// Percent of one core; `None` until two samples exist.
    pub cpu: Option<f64>,
    pub rss: u64,
}

impl Usage {
    pub const UNMEASURED: Usage = Usage { cpu: None, rss: 0 };
}

impl Add for Usage {
    type Output = Usage;
    fn add(self, other: Usage) -> Usage {
        let cpu = match (self.cpu, other.cpu) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
        };
        Usage {
            cpu,
            rss: self.rss + other.rss,
        }
    }
}

impl std::iter::Sum for Usage {
    fn sum<I: Iterator<Item = Usage>>(iter: I) -> Usage {
        iter.fold(Usage::UNMEASURED, Add::add)
    }
}

/// `ps` cumulative CPU time in ms: macOS `MMM:SS.cc` (minutes unbounded), also `[dd-][hh:]mm:ss`.
pub fn parse_cpu_time(text: &str) -> Option<u64> {
    let (days, rest) = match text.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().ok()?, rest),
        None => (0, text),
    };
    let mut seconds = 0.0;
    for part in rest.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(days * 86_400_000 + (seconds * 1000.0).round() as u64)
}

/// Split off the first whitespace-separated token.
fn token(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

fn parse_ps_line(line: &str) -> Option<Proc> {
    let (pid, rest) = token(line)?;
    let (ppid, rest) = token(rest)?;
    let (time, rest) = token(rest)?;
    let (rss, rest) = token(rest)?;
    Some(Proc {
        pid: pid.parse().ok()?,
        ppid: ppid.parse().ok()?,
        cpu_ms: parse_cpu_time(time)?,
        rss: rss.parse::<u64>().ok()? * 1024,
        comm: rest.trim().to_string(),
    })
}

/// Rows of `ps -o pid=,ppid=,time=,rss=[,comm=]`; unreadable rows are skipped.
pub fn parse_ps(text: &str) -> ProcTable {
    text.lines()
        .filter_map(parse_ps_line)
        .map(|p| (p.pid, p))
        .collect()
}

pub fn children_map(procs: &ProcTable) -> HashMap<u32, Vec<u32>> {
    let mut kids: HashMap<u32, Vec<u32>> = HashMap::new();
    for p in procs.values().filter(|p| p.pid != p.ppid) {
        kids.entry(p.ppid).or_default().push(p.pid);
    }
    kids
}

/// `root` and every process below it.
pub fn subtree(root: u32, kids: &HashMap<u32, Vec<u32>>) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        out.push(pid);
        if let Some(children) = kids.get(&pid) {
            stack.extend(children);
        }
    }
    out
}

/// RSS summed; CPU from per-pid deltas since the previous sample, so an exited child can't cancel
/// its siblings, a new pid counts from zero, and a reused pid never goes negative.
pub fn usage_of(
    pids: &[u32],
    procs: &ProcTable,
    prev_cpu: Option<&HashMap<u32, u64>>,
    dt: Option<Duration>,
) -> Usage {
    let live = || pids.iter().filter_map(|pid| procs.get(pid));
    let rss = live().map(|p| p.rss).sum();
    let cpu = match (prev_cpu, dt) {
        (Some(prev), Some(dt)) if !dt.is_zero() => {
            let busy_ms: u64 = live()
                .map(|p| {
                    p.cpu_ms
                        .saturating_sub(prev.get(&p.pid).copied().unwrap_or(0))
                })
                .sum();
            Some(busy_ms as f64 / 1000.0 / dt.as_secs_f64() * 100.0)
        }
        _ => None,
    };
    Usage { cpu, rss }
}

/// Group helpers under their app: `/Applications/Google Chrome.app/.../Helper` → `Google Chrome`.
pub fn app_name(comm: &str) -> String {
    let path = comm.split_once(".app/").map_or(comm, |(app, _)| app);
    match path.rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => "?".to_string(),
    }
}

/// Where the process table comes from: `ps` on macOS, sysinfo on Linux.
pub trait ProcSource {
    fn read(&mut self) -> std::io::Result<ProcTable>;
}

type Runner = Box<dyn FnMut(&[&str]) -> std::io::Result<String> + Send>;
type Namer = Box<dyn FnMut(&[u32]) -> HashMap<u32, String> + Send>;

/// macOS: `ps` is setuid root, so it also sees the numbers of system daemons (WindowServer, mds…) that
/// an unprivileged reader can't. One `ps -A` per sample; names (only used to label "Outside herdr") are
/// read in-process with sysinfo, which can read every process's path, and only for pids not seen before.
pub struct PsSource {
    run: Runner,
    name: Namer,
    names: HashMap<u32, String>,
}

impl PsSource {
    pub fn new() -> Self {
        let mut system = sysinfo::System::new();
        Self::with(
            |args: &[&str]| {
                let out = std::process::Command::new("ps").args(args).output()?;
                Ok(String::from_utf8_lossy(&out.stdout).into_owned())
            },
            move |pids: &[u32]| names_from_system(&mut system, pids),
        )
    }

    /// For tests: `run` gets the `ps` arguments and returns its stdout; `name` gets new pids.
    pub fn with(
        run: impl FnMut(&[&str]) -> std::io::Result<String> + Send + 'static,
        name: impl FnMut(&[u32]) -> HashMap<u32, String> + Send + 'static,
    ) -> Self {
        PsSource {
            run: Box::new(run),
            name: Box::new(name),
            names: HashMap::new(),
        }
    }
}

impl Default for PsSource {
    fn default() -> Self {
        Self::new()
    }
}

fn names_from_system(system: &mut sysinfo::System, pids: &[u32]) -> HashMap<u32, String> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
    let pids: Vec<Pid> = pids.iter().map(|&p| Pid::from_u32(p)).collect();
    let kind = ProcessRefreshKind::nothing()
        .with_exe(UpdateKind::Always)
        .without_tasks();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, kind);
    pids.iter()
        .filter_map(|pid| {
            let p = system.process(*pid)?;
            let name = p.exe().map_or_else(
                || p.name().to_string_lossy().into_owned(),
                |e| e.display().to_string(),
            );
            Some((pid.as_u32(), name))
        })
        .collect()
}

impl ProcSource for PsSource {
    fn read(&mut self) -> std::io::Result<ProcTable> {
        let mut procs = parse_ps(&(self.run)(&["-A", "-o", "pid=,ppid=,time=,rss="])?);
        let new: Vec<u32> = procs
            .keys()
            .copied()
            .filter(|p| !self.names.contains_key(p))
            .collect();
        if !new.is_empty() {
            self.names.extend((self.name)(&new));
        }
        self.names.retain(|pid, _| procs.contains_key(pid));
        for p in procs.values_mut() {
            p.comm = self.names.get(&p.pid).cloned().unwrap_or_default();
        }
        Ok(procs)
    }
}

/// Linux (and anywhere `/proc`-style access covers all processes): the sysinfo crate.
pub struct SysinfoSource {
    system: sysinfo::System,
}

impl SysinfoSource {
    pub fn new() -> Self {
        SysinfoSource {
            system: sysinfo::System::new(),
        }
    }
}

impl Default for SysinfoSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcSource for SysinfoSource {
    fn read(&mut self) -> std::io::Result<ProcTable> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        // Without tasks: on Linux threads would otherwise be listed as processes and double-count memory.
        let kind = ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .without_tasks();
        self.system
            .refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        Ok(self
            .system
            .processes()
            .values()
            .filter(|p| p.thread_kind().is_none())
            .map(|p| {
                let pid = p.pid().as_u32();
                let comm = p.exe().map_or_else(
                    || p.name().to_string_lossy().into_owned(),
                    |e| e.display().to_string(),
                );
                let proc_ = Proc {
                    pid,
                    ppid: p.parent().map_or(0, |pp| pp.as_u32()),
                    cpu_ms: p.accumulated_cpu_time(),
                    rss: p.memory(),
                    comm,
                };
                (pid, proc_)
            })
            .collect())
    }
}

/// The right source for this platform.
#[cfg(target_os = "macos")]
pub fn platform_source() -> PsSource {
    PsSource::new()
}

/// The right source for this platform.
#[cfg(not(target_os = "macos"))]
pub fn platform_source() -> SysinfoSource {
    SysinfoSource::new()
}
