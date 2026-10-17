//! One sample: herdr's layout + the process table → a tree with usage.

use std::collections::HashMap;
use std::time::Instant;

use crate::herdr::client::{Error, Herdr};
use crate::procs::{ProcSource, Usage};
use crate::tree::{Node, build_tree};

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub nodes: Vec<Node>,
    /// Sum of all herdr panes (not "Outside herdr").
    pub total: Usage,
}

pub struct Sampler<H, P> {
    herdr: H,
    procs: P,
    own_pid: u32,
    prev_cpu: Option<HashMap<u32, u64>>,
    prev_at: Option<Instant>,
}

impl<H: Herdr, P: ProcSource> Sampler<H, P> {
    /// `own_pid`'s process tree (this popup) is left out of every row.
    pub fn new(herdr: H, procs: P, own_pid: u32) -> Self {
        Sampler {
            herdr,
            procs,
            own_pid,
            prev_cpu: None,
            prev_at: None,
        }
    }

    /// Takes one sample; CPU appears from the second sample on. `clock` is read right after the process
    /// table, so the time between herdr's answers doesn't count towards the CPU interval.
    pub fn sample(&mut self, clock: impl FnOnce() -> Instant) -> Result<Frame, Error> {
        let snapshot = self.herdr.snapshot()?;
        let infos = snapshot
            .panes
            .iter()
            // A pane can close between the two calls: it still shows, without numbers.
            .map(|p| (p.pane_id.clone(), self.herdr.process_info(&p.pane_id).ok()))
            .collect();
        let procs = self
            .procs
            .read()
            .map_err(|e| Error::Io(format!("process table: {e}")))?;
        let now = clock();
        let dt = self.prev_at.map(|t| now.saturating_duration_since(t));
        let (nodes, total) = build_tree(
            &snapshot,
            &infos,
            &procs,
            self.prev_cpu.as_ref(),
            dt,
            self.own_pid,
        );
        self.prev_cpu = Some(procs.values().map(|p| (p.pid, p.cpu_ms)).collect());
        self.prev_at = Some(now);
        Ok(Frame { nodes, total })
    }
}
