//! Display formatting: sizes, CPU, sparklines, width-aware truncation.

use unicode_width::UnicodeWidthChar;

const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
/// A wobble smaller than this share of the row's peak draws flat (RSS noise isn't a trend).
const TREND_MIN_SPAN: f64 = 0.05;

const KIB: f64 = 1024.0;
const MIB: f64 = KIB * 1024.0;
const GIB: f64 = MIB * 1024.0;

/// `784 KB`, `180.3 MB`, `1.58 GB`.
pub fn format_bytes(n: u64) -> String {
    let n = n as f64;
    if n < MIB {
        format!("{:.0} KB", n / KIB)
    } else if n < GIB {
        format!("{:.1} MB", n / MIB)
    } else {
        format!("{:.2} GB", n / GIB)
    }
}

/// `67.2%`, or `…` while CPU isn't measured yet.
pub fn format_cpu(cpu: Option<f64>) -> String {
    match cpu {
        Some(c) => format!("{c:.1}%"),
        None => "…".to_string(),
    }
}

/// The last `width` values as block characters, scaled between the row's own min and max.
pub fn sparkline(values: &[u64], width: usize) -> String {
    let vals = &values[values.len().saturating_sub(width)..];
    let (Some(&lo), Some(&hi)) = (vals.iter().min(), vals.iter().max()) else {
        return String::new();
    };
    let (lo, hi) = (lo as f64, hi as f64);
    let span = (hi - lo).max(hi * TREND_MIN_SPAN);
    if span <= 0.0 {
        return SPARK[0].to_string().repeat(vals.len());
    }
    let top = (SPARK.len() - 1) as f64;
    vals.iter()
        .map(|&v| SPARK[((v as f64 - lo) / span * top).round_ties_even() as usize])
        .collect()
}

fn char_width(ch: char) -> usize {
    ch.width().unwrap_or(0)
}

/// Terminal columns `s` occupies (wide CJK = 2, combining marks = 0).
pub fn text_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Cut `s` to at most `width` columns, ending in `…` when cut; never splits a character.
pub fn truncate(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text_width(s) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let w = char_width(ch);
        if used + w > width - 1 {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}
