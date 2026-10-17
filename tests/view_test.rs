mod common;

use std::collections::HashMap;
use std::time::Duration;

use common::NO_PID;
use herdr_popup_resource_manager::app::{Key, Model, Msg, update};
use herdr_popup_resource_manager::sampler::Frame;
use herdr_popup_resource_manager::tree::build_tree;
use herdr_popup_resource_manager::view::{palette, view};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;

/// A frame with CPU: claude in w9:p5 used 1.2 s of CPU over 1 s (hot), everything else idle.
fn busy_frame() -> Frame {
    let procs = common::procs();
    let mut prev: HashMap<u32, u64> = procs.values().map(|p| (p.pid, p.cpu_ms)).collect();
    *prev.get_mut(&12339).unwrap() -= 1_200;
    let (nodes, total) = build_tree(
        &common::snapshot(),
        &common::infos(),
        &procs,
        Some(&prev),
        Some(Duration::from_secs(1)),
        NO_PID,
    );
    Frame { nodes, total }
}

fn model(w: u16, h: u16) -> Model {
    let mut m = Model::new(18 << 30, 11, (w, h));
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(busy_frame()),
        },
    );
    m
}

fn render(m: &Model) -> Terminal<TestBackend> {
    let (w, h) = m.viewport;
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| view(m, f)).unwrap();
    terminal
}

fn line(t: &Terminal<TestBackend>, y: u16) -> String {
    let buf = t.backend().buffer();
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

fn find(t: &Terminal<TestBackend>, text: &str) -> (u16, u16) {
    let buf = t.backend().buffer();
    for y in 0..buf.area.height {
        if let Some(col) = line(t, y).find(text) {
            let x = line(t, y)[..col].chars().count() as u16;
            return (x, y);
        }
    }
    panic!("{text:?} not on screen:\n{}", t.backend());
}

#[test]
fn wide_popup() {
    insta::assert_snapshot!(render(&model(110, 24)).backend());
}

#[test]
fn narrow_popup_hides_the_trend() {
    insta::assert_snapshot!(render(&model(60, 14)).backend());
}

#[test]
fn very_narrow_popup_keeps_the_title_whole() {
    let t = render(&model(45, 12));
    assert!(line(&t, 0).contains(" popup-resource-manager "));
    assert!(!line(&t, 0).contains("CPU 100%"));
}

#[test]
fn tiny_popup_asks_to_be_enlarged() {
    let t = render(&Model::new(1, 1, (30, 6)));
    assert!(line(&t, 0).starts_with("Enlarge the popup"));
}

#[test]
fn before_the_first_sample_it_says_so_and_errors_replace_that() {
    let mut m = Model::new(18 << 30, 11, (80, 12));
    assert!(line(&render(&m), 3).contains("Sampling…"));
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Err("herdr socket /x: connection refused".into()),
        },
    );
    let t = render(&m);
    assert!(line(&t, 3).contains("connection refused"));
}

#[test]
fn the_confirm_question_replaces_the_key_hints() {
    let mut m = model(110, 24);
    m.selected = Some("w9:pD".into());
    update(&mut m, Msg::Key(Key::Char('x')));
    assert!(line(&render(&m), 22).contains("close w9:pD (shell)? y/N"));
}

#[test]
fn messages_replace_the_key_hints() {
    let mut m = model(110, 24);
    update(&mut m, Msg::Closed(Err("pane.close: refused".into())));
    assert!(line(&render(&m), 22).contains("pane.close: refused"));
}

#[test]
fn hot_cpu_is_highlighted_and_idle_is_not() {
    let t = render(&model(110, 24));
    let buf = t.backend().buffer();
    let (x, y) = find(&t, "120.0%");
    assert_eq!(buf[(x, y)].fg, palette::HOT);
    let (x, y) = find(&t, "0.0%");
    assert_ne!(buf[(x, y)].fg, palette::HOT);
}

#[test]
fn status_dots_follow_the_agent_status() {
    let t = render(&model(110, 24));
    let buf = t.backend().buffer();
    let dot = |label: &str| {
        let (x, y) = find(&t, &format!("● {label}"));
        buf[(x, y)].fg
    };
    assert_eq!(dot("claude  Review commits"), palette::GREEN); // idle
    assert_eq!(dot("claude  Plan the next"), palette::ORANGE); // working
    assert_eq!(dot("shell"), palette::GREY);
}

#[test]
fn the_selected_row_is_reversed() {
    let mut m = model(110, 24);
    m.selected = Some("w9".into());
    let t = render(&m);
    let (_, y) = find(&t, "[2] notes");
    let buf = t.backend().buffer();
    assert!(buf[(5, y)].modifier.contains(Modifier::REVERSED));
    assert!(!buf[(5, y + 1)].modifier.contains(Modifier::REVERSED));
}
