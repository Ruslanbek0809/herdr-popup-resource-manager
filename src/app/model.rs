use std::collections::{HashMap, HashSet, VecDeque};

use crate::herdr::client::Target;
use crate::sampler::Frame;
use crate::tree::{Row, flatten};

/// Samples kept per workspace for the memory trend.
pub const TREND_LEN: usize = 120;
/// Lines the frame uses besides the list: border, header, two rules, total, footer, border.
const CHROME_ROWS: u16 = 7;
/// Screen line of the first list row (below the border, header and rule).
pub const LIST_TOP: u16 = 3;
/// Screen column where list rows start (border + one space).
pub const LIST_LEFT: u16 = 2;
/// Below this size the popup only asks to be enlarged.
pub const MIN_W: u16 = 40;
pub const MIN_H: u16 = 8;

/// How many list rows fit in a popup `height` lines tall.
pub fn list_rows(height: u16) -> usize {
    usize::from(height.saturating_sub(CHROME_ROWS)).max(1)
}

/// Keys the app reacts to (the runtime maps terminal events onto these).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    PageUp,
    PageDown,
    Enter,
    Esc,
    Char(char),
}

/// Mouse input the app reacts to. `col` and `row` are screen cells from the top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mouse {
    Click { col: u16, row: u16 },
    ScrollUp,
    ScrollDown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// A sample from the sampler thread. `generation` is the one it was asked for (see [`Cmd::SampleNow`]).
    Sampled {
        generation: u64,
        result: Result<Frame, String>,
    },
    Key(Key),
    Mouse(Mouse),
    Resized {
        w: u16,
        h: u16,
    },
    Focused(Result<(), String>),
    Closed(Result<(), String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    /// Sample right away; frames from older generations (taken before a close) are ignored.
    SampleNow {
        generation: u64,
    },
    Focus(Target),
    ClosePane(String),
    Quit,
}

/// A pending "close this pane?" question.
#[derive(Debug, Clone, PartialEq)]
pub struct Confirm {
    pub pane_id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub frame: Option<Frame>,
    pub folded: HashSet<String>,
    /// Id of the selected row; `last_index` keeps its place when the row disappears.
    pub selected: Option<String>,
    pub last_index: usize,
    pub scroll: usize,
    /// Terminal size (columns, lines).
    pub viewport: (u16, u16),
    pub confirm: Option<Confirm>,
    /// From a key press (failed focus/close, hints); cleared by the next key.
    pub message: Option<String>,
    /// From sampling; cleared by the next good sample.
    pub sample_error: Option<String>,
    /// Memory (RSS) per top-level row, oldest first.
    pub history: HashMap<String, VecDeque<u64>>,
    pub generation: u64,
    pub ram: u64,
    pub cores: usize,
}

impl Model {
    pub fn new(ram: u64, cores: usize, viewport: (u16, u16)) -> Model {
        Model {
            frame: None,
            folded: HashSet::new(),
            selected: None,
            last_index: 0,
            scroll: 0,
            viewport,
            confirm: None,
            message: None,
            sample_error: None,
            history: HashMap::new(),
            generation: 0,
            ram,
            cores,
        }
    }

    /// The visible rows, in display order.
    pub fn rows(&self) -> Vec<Row<'_>> {
        self.frame
            .as_ref()
            .map(|f| flatten(&f.nodes, &self.folded))
            .unwrap_or_default()
    }

    /// Index of the selected row; if it vanished, the row now at its old place.
    pub fn selected_index(&self) -> usize {
        let rows = self.rows();
        rows.iter()
            .position(|r| Some(&r.node.id) == self.selected.as_ref())
            .unwrap_or_else(|| self.last_index.min(rows.len().saturating_sub(1)))
    }

    pub fn visible_rows(&self) -> usize {
        list_rows(self.viewport.1)
    }
}
