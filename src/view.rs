//! Drawing the model with ratatui. Read-only: the view never changes state.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

use crate::app::{LIST_LEFT, LIST_TOP, MIN_H, MIN_W, Model, list_rows};
use crate::format::{format_bytes, format_cpu, sparkline, text_width, truncate};
use crate::tree::{NodeKind, Row};

pub const TITLE: &str = "popup-resource-manager";
/// CPU at or above this (percent of one core) is highlighted.
const HOT_CPU: f64 = 50.0;
const CPU_W: u16 = 7;
const MEM_W: u16 = 10;
const GAP: u16 = 2;
const TREND_MAX_W: u16 = 30;
/// Below this inner width the trend column is dropped.
const TREND_MIN_INNER: u16 = 70;

/// 256-colour accents on the terminal's own background, so light and dark themes both work.
pub mod palette {
    use ratatui::style::Color;
    pub const ACCENT: Color = Color::Indexed(141);
    pub const TREND: Color = Color::Indexed(115);
    pub const HOT: Color = Color::Indexed(222);
    pub const GREEN: Color = Color::Indexed(114);
    pub const ORANGE: Color = Color::Indexed(215);
    pub const PINK: Color = Color::Indexed(211);
    pub const GREY: Color = Color::Indexed(245);
    pub const ERROR: Color = Color::Indexed(203);
}

fn fg(color: Color) -> Style {
    Style::default().fg(color)
}

fn status_color(status: Option<&str>) -> Color {
    match status {
        Some("working") => palette::ORANGE,
        Some("blocked") => palette::PINK,
        Some("idle" | "done") => palette::GREEN,
        _ => palette::GREY,
    }
}

/// Column positions for a popup `area.width` wide (all x are absolute).
struct Columns {
    left: u16,
    inner: u16,
    name_w: u16,
    cpu_right: u16,
    mem_right: u16,
    trend_x: u16,
    trend_w: u16,
}

impl Columns {
    fn new(area: Rect) -> Columns {
        let left = area.x + LIST_LEFT;
        let right = area.x + area.width - 3;
        let inner = right - left + 1;
        let trend_w = if inner >= TREND_MIN_INNER {
            TREND_MAX_W.min(inner / 4)
        } else {
            0
        };
        let trend_x = right + 1 - trend_w;
        let mem_right = if trend_w > 0 {
            trend_x - GAP
        } else {
            right + 1
        };
        let cpu_right = mem_right - MEM_W - GAP;
        let name_w = cpu_right - CPU_W - GAP - left;
        Columns {
            left,
            inner,
            name_w,
            cpu_right,
            mem_right,
            trend_x,
            trend_w,
        }
    }
}

/// Writes `text` so it ends just before column `right`.
fn put_right(buf: &mut Buffer, right: u16, y: u16, text: &str, style: Style) {
    let w = text_width(text) as u16;
    buf.set_string(right.saturating_sub(w), y, text, style);
}

pub fn view(model: &Model, frame: &mut Frame) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    if area.width < MIN_W || area.height < MIN_H {
        buf.set_string(
            area.x,
            area.y,
            truncate("Enlarge the popup", area.width.into()),
            Style::default(),
        );
        return;
    }
    Block::default()
        .borders(Borders::ALL)
        .border_style(fg(palette::GREY))
        .render(area, buf);
    buf.set_string(
        area.x + 2,
        area.y,
        format!(" {TITLE} "),
        fg(palette::ACCENT).add_modifier(Modifier::BOLD),
    );
    let note = format!(" CPU 100% = 1 core of {} ", model.cores);
    if text_width(&note) + text_width(TITLE) + 8 <= area.width.into() {
        put_right(
            buf,
            area.x + area.width - 2,
            area.y,
            &note,
            fg(palette::GREY),
        );
    }

    let c = Columns::new(area);
    let dim = fg(palette::GREY);
    let rule = "─".repeat(c.inner.into());
    buf.set_string(c.left, area.y + 1, "Name", dim);
    put_right(buf, c.cpu_right, area.y + 1, "CPU", dim);
    put_right(buf, c.mem_right, area.y + 1, "Memory", dim);
    if c.trend_w > 0 {
        buf.set_string(c.trend_x, area.y + 1, "Memory trend", dim);
    }
    buf.set_string(c.left, area.y + 2, &rule, dim);

    let list_top = area.y + LIST_TOP;
    if model.frame.is_none() {
        let (text, style) = match &model.sample_error {
            Some(e) => (e.as_str(), fg(palette::ERROR)),
            None => ("Sampling…", dim),
        };
        buf.set_string(c.left, list_top, truncate(text, c.inner.into()), style);
    }
    let rows = model.rows();
    let selected = model.selected_index();
    for (i, row) in rows
        .iter()
        .enumerate()
        .skip(model.scroll)
        .take(list_rows(area.height))
    {
        let y = list_top + (i - model.scroll) as u16;
        draw_row(buf, model, &c, y, row);
        if i == selected {
            buf.set_style(
                Rect::new(c.left - 1, y, c.inner + 2, 1),
                Style::default().add_modifier(Modifier::REVERSED),
            );
        }
    }

    let y = area.y + area.height - 4;
    buf.set_string(c.left, y, &rule, dim);
    if let Some(frame) = &model.frame {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        buf.set_string(c.left, y + 1, "Total in herdr", bold);
        put_right(buf, c.cpu_right, y + 1, &format_cpu(frame.total.cpu), bold);
        put_right(
            buf,
            c.mem_right,
            y + 1,
            &format_bytes(frame.total.rss),
            bold,
        );
        if c.trend_w > 0 && model.ram > 0 {
            let share = format!(
                "~{:.0}% of system RAM",
                frame.total.rss as f64 / model.ram as f64 * 100.0
            );
            put_right(
                buf,
                c.trend_x + c.trend_w,
                y + 1,
                &truncate(&share, c.trend_w.into()),
                dim,
            );
        }
    }
    draw_footer(buf, model, &c, area.y + area.height - 2);
}

fn draw_row(buf: &mut Buffer, model: &Model, c: &Columns, y: u16, row: &Row) {
    let n = row.node;
    let indent = (row.depth * 2) as u16;
    let mut x = c.left + indent;
    let mut width = c.name_w.saturating_sub(indent);
    let marker = if row.foldable {
        Some((
            if model.folded.contains(&n.id) {
                "▸ "
            } else {
                "▾ "
            },
            palette::GREY,
        ))
    } else if n.kind == NodeKind::Pane {
        Some(("● ", status_color(n.status.as_deref())))
    } else if n.kind == NodeKind::App {
        Some(("· ", palette::GREY))
    } else {
        None
    };
    if let Some((text, color)) = marker {
        buf.set_string(x, y, text, fg(color));
        x += 2;
        width = width.saturating_sub(2);
    }
    let style = match n.kind {
        NodeKind::Workspace => fg(palette::ACCENT).add_modifier(Modifier::BOLD),
        NodeKind::Outside => Style::default().add_modifier(Modifier::BOLD),
        _ => Style::default(),
    };
    let label = truncate(&n.label, width.into());
    let used = text_width(&label) as u16;
    buf.set_string(x, y, &label, style);
    if !n.detail.is_empty() && width.saturating_sub(used) > 4 {
        let detail = truncate(&n.detail, (width - used - 2).into());
        buf.set_string(x + used + 2, y, detail, fg(palette::GREY));
    }
    let hot = n.usage.cpu.is_some_and(|cpu| cpu >= HOT_CPU);
    let cpu_style = if hot {
        fg(palette::HOT)
    } else {
        Style::default()
    };
    put_right(buf, c.cpu_right, y, &format_cpu(n.usage.cpu), cpu_style);
    put_right(
        buf,
        c.mem_right,
        y,
        &format_bytes(n.usage.rss),
        Style::default(),
    );
    if c.trend_w > 0
        && row.depth == 0
        && let Some(history) = model.history.get(&n.id)
    {
        let values: Vec<u64> = history.iter().copied().collect();
        buf.set_string(
            c.trend_x,
            y,
            sparkline(&values, c.trend_w.into()),
            fg(palette::TREND),
        );
    }
}

fn draw_footer(buf: &mut Buffer, model: &Model, c: &Columns, y: u16) {
    let width = c.inner.into();
    if let Some(confirm) = &model.confirm {
        let text = format!("close {} ({})? y/N", confirm.pane_id, confirm.label);
        buf.set_string(
            c.left,
            y,
            truncate(&text, width),
            fg(palette::ORANGE).add_modifier(Modifier::BOLD),
        );
        return;
    }
    if let Some(message) = model.message.as_ref().or(model.sample_error.as_ref()) {
        buf.set_string(c.left, y, truncate(message, width), fg(palette::ERROR));
        return;
    }
    let keys = [
        ("↑↓", "move"),
        ("space", "fold"),
        ("enter", "focus"),
        ("x", "close pane"),
        ("q", "quit"),
    ];
    let mut x = c.left;
    for (i, (key, what)) in keys.iter().enumerate() {
        let sep = if i + 1 < keys.len() { "  ·  " } else { "" };
        let part_w = (text_width(key) + 1 + text_width(what) + text_width(sep)) as u16;
        if x + part_w > c.left + c.inner {
            break;
        }
        buf.set_string(x, y, key, fg(palette::ACCENT));
        buf.set_string(
            x + text_width(key) as u16 + 1,
            y,
            format!("{what}{sep}"),
            fg(palette::GREY),
        );
        x += part_w;
    }
}
