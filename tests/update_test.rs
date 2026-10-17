mod common;

use common::NO_PID;
use herdr_popup_resource_manager::app::{Cmd, Key, Model, Msg, TREND_LEN, update};
use herdr_popup_resource_manager::herdr::client::Target;
use herdr_popup_resource_manager::sampler::Frame;
use herdr_popup_resource_manager::tree::build_tree;

fn fixture_frame() -> Frame {
    let (nodes, total) = build_tree(
        &common::snapshot(),
        &common::infos(),
        &common::procs(),
        None,
        None,
        NO_PID,
    );
    Frame { nodes, total }
}

/// A model that has received one good sample, in a 110×40 popup.
fn model() -> Model {
    let mut m = Model::new(18 << 30, 11, (110, 40));
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(fixture_frame()),
        },
    );
    m
}

/// Selects a row the way moving there would: id and its index.
fn select(m: &mut Model, id: &str) {
    m.selected = Some(id.to_string());
    m.last_index = m.selected_index();
}

fn key(m: &mut Model, k: Key) -> Vec<Cmd> {
    update(m, Msg::Key(k))
}

#[test]
fn enter_on_a_pane_focuses_it_and_quits_once_herdr_confirms() {
    let mut m = model();
    select(&mut m, "w9:p1");
    assert_eq!(
        key(&mut m, Key::Enter),
        [Cmd::Focus(Target::Pane("w9:p1".into()))]
    );
    assert_eq!(update(&mut m, Msg::Focused(Ok(()))), [Cmd::Quit]);
}

#[test]
fn enter_focuses_tabs_and_workspaces_and_ignores_outside_rows() {
    let mut m = model();
    select(&mut m, "w9:t5");
    assert_eq!(
        key(&mut m, Key::Enter),
        [Cmd::Focus(Target::Tab("w9:t5".into()))]
    );
    select(&mut m, "w8");
    assert_eq!(
        key(&mut m, Key::Enter),
        [Cmd::Focus(Target::Workspace("w8".into()))]
    );
    select(&mut m, "outside");
    assert_eq!(key(&mut m, Key::Enter), []);
}

#[test]
fn a_failed_focus_keeps_the_popup_open_with_the_reason_until_the_next_key() {
    let mut m = model();
    select(&mut m, "w9:p1");
    key(&mut m, Key::Enter);
    assert_eq!(
        update(&mut m, Msg::Focused(Err("pane.focus: nope".into()))),
        []
    );
    assert_eq!(m.message.as_deref(), Some("pane.focus: nope"));
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(fixture_frame()),
        },
    );
    assert!(m.message.is_some(), "a new sample doesn't hide it");
    key(&mut m, Key::Char('j'));
    assert_eq!(m.message, None);
}

#[test]
fn x_asks_first_and_only_y_closes() {
    let mut m = model();
    select(&mut m, "w9:pD");
    assert_eq!(key(&mut m, Key::Char('x')), []);
    assert_eq!(
        m.confirm.as_ref().map(|c| c.pane_id.as_str()),
        Some("w9:pD")
    );
    assert_eq!(key(&mut m, Key::Char('n')), []);
    assert!(m.confirm.is_none());
    key(&mut m, Key::Char('x'));
    assert_eq!(
        key(&mut m, Key::Char('y')),
        [Cmd::ClosePane("w9:pD".into())]
    );
    assert!(m.confirm.is_none());
}

#[test]
fn x_on_a_workspace_row_explains_instead_of_asking() {
    let mut m = model();
    select(&mut m, "w9");
    key(&mut m, Key::Char('x'));
    assert!(m.confirm.is_none());
    assert!(m.message.is_some());
}

#[test]
fn a_failed_close_shows_its_error_until_the_next_key() {
    let mut m = model();
    select(&mut m, "w9:pD");
    key(&mut m, Key::Char('x'));
    key(&mut m, Key::Char('y'));
    assert_eq!(
        update(&mut m, Msg::Closed(Err("pane.close: refused".into()))),
        []
    );
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(fixture_frame()),
        },
    );
    assert_eq!(m.message.as_deref(), Some("pane.close: refused"));
}

#[test]
fn after_a_close_frames_sampled_before_it_are_ignored() {
    let mut m = model();
    select(&mut m, "w9:pD");
    key(&mut m, Key::Char('x'));
    key(&mut m, Key::Char('y'));
    let cmds = update(&mut m, Msg::Closed(Ok(())));
    assert_eq!(cmds, [Cmd::SampleNow { generation: 1 }]);

    let mut closed = fixture_frame();
    let ws = closed.nodes.iter_mut().find(|n| n.id == "w9").unwrap();
    ws.children.retain(|t| t.id != "w9:t7");

    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(fixture_frame()),
        },
    ); // stale: still has w9:pD
    assert!(
        m.rows().iter().any(|r| r.node.id == "w9:pD"),
        "stale frame ignored, old frame kept"
    );
    update(
        &mut m,
        Msg::Sampled {
            generation: 1,
            result: Ok(closed),
        },
    );
    assert!(!m.rows().iter().any(|r| r.node.id == "w9:pD"));
}

#[test]
fn selection_stays_at_the_same_place_when_its_row_vanishes() {
    let mut m = model();
    select(&mut m, "w9:pD");
    let before = m.selected_index();
    let mut closed = fixture_frame();
    closed
        .nodes
        .iter_mut()
        .find(|n| n.id == "w9")
        .unwrap()
        .children
        .retain(|t| t.id != "w9:t7");
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(closed),
        },
    );
    assert_eq!(
        m.selected_index(),
        before,
        "w9:t7 + w9:pD vanished; the row now at that index is selected"
    );
    assert_eq!(
        m.selected.as_deref(),
        Some(m.rows()[before].node.id.as_str())
    );
}

#[test]
fn sampling_errors_keep_the_last_frame_and_clear_on_the_next_good_sample() {
    let mut m = model();
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Err("socket gone".into()),
        },
    );
    assert_eq!(m.sample_error.as_deref(), Some("socket gone"));
    assert!(m.frame.is_some());
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(fixture_frame()),
        },
    );
    assert_eq!(m.sample_error, None);
}

#[test]
fn moving_and_folding() {
    let mut m = model();
    select(&mut m, "w8");
    key(&mut m, Key::Down);
    assert_eq!(m.selected.as_deref(), Some("w8:t1"));
    key(&mut m, Key::Char('k'));
    key(&mut m, Key::Char(' '));
    assert!(m.folded.contains("w8"));
    key(&mut m, Key::Char('j'));
    assert_eq!(m.selected.as_deref(), Some("w9"));
    key(&mut m, Key::Up);
    key(&mut m, Key::Char(' '));
    assert!(!m.folded.contains("w8"));
    key(&mut m, Key::PageDown);
    assert_eq!(m.selected_index(), 10);
    key(&mut m, Key::Up);
    key(&mut m, Key::PageUp);
    assert_eq!(m.selected_index(), 0);
}

#[test]
fn q_and_esc_quit() {
    let mut m = model();
    assert_eq!(key(&mut m, Key::Char('q')), [Cmd::Quit]);
    assert_eq!(key(&mut m, Key::Esc), [Cmd::Quit]);
}

#[test]
fn the_trend_keeps_the_last_samples_per_workspace_and_forgets_closed_ones() {
    let mut m = model();
    for _ in 0..TREND_LEN + 5 {
        update(
            &mut m,
            Msg::Sampled {
                generation: 0,
                result: Ok(fixture_frame()),
            },
        );
    }
    assert_eq!(m.history["w9"].len(), TREND_LEN);
    let mut gone = fixture_frame();
    gone.nodes.retain(|n| n.id != "w8");
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(gone),
        },
    );
    assert!(!m.history.contains_key("w8"));
}

#[test]
fn scrolling_keeps_the_selection_visible() {
    let mut m = Model::new(18 << 30, 11, (110, 12)); // 12 rows tall → 5 list rows
    update(
        &mut m,
        Msg::Sampled {
            generation: 0,
            result: Ok(fixture_frame()),
        },
    );
    assert_eq!(m.visible_rows(), 5);
    for _ in 0..7 {
        key(&mut m, Key::Down);
    }
    assert_eq!((m.selected_index(), m.scroll), (7, 3));
    update(&mut m, Msg::Resized { w: 110, h: 40 });
    assert_eq!(m.scroll, 0, "everything fits again");
}
