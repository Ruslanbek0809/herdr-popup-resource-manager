use std::collections::{HashSet, VecDeque};

use super::model::{Cmd, Confirm, Key, Model, Msg, TREND_LEN};
use crate::herdr::client::Target;
use crate::sampler::Frame;
use crate::tree::NodeKind;

const PAGE: usize = 10;

/// The only place state changes. Pure: no IO, no clock, no terminal.
pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let cmds = match msg {
        Msg::Sampled { generation, result } => {
            if generation >= model.generation {
                match result {
                    Ok(frame) => on_frame(model, frame),
                    Err(e) => model.sample_error = Some(e),
                }
            }
            vec![]
        }
        Msg::Key(key) => on_key(model, key),
        Msg::Resized { w, h } => {
            model.viewport = (w, h);
            vec![]
        }
        Msg::Focused(Ok(())) => vec![Cmd::Quit],
        Msg::Closed(Ok(())) => {
            model.generation += 1;
            vec![Cmd::SampleNow {
                generation: model.generation,
            }]
        }
        Msg::Focused(Err(e)) | Msg::Closed(Err(e)) => {
            model.message = Some(e);
            vec![]
        }
    };
    settle_selection(model);
    cmds
}

fn on_frame(model: &mut Model, frame: Frame) {
    model.sample_error = None;
    let top: HashSet<&str> = frame.nodes.iter().map(|n| n.id.as_str()).collect();
    model.history.retain(|id, _| top.contains(id.as_str()));
    for node in &frame.nodes {
        let ring = model
            .history
            .entry(node.id.clone())
            .or_insert_with(|| VecDeque::with_capacity(TREND_LEN));
        if ring.len() == TREND_LEN {
            ring.pop_front();
        }
        ring.push_back(node.usage.rss);
    }
    model.frame = Some(frame);
}

fn on_key(model: &mut Model, key: Key) -> Vec<Cmd> {
    model.message = None;
    if let Some(confirm) = model.confirm.take() {
        return match key {
            Key::Char('y' | 'Y') => vec![Cmd::ClosePane(confirm.pane_id)],
            _ => vec![],
        };
    }
    let ids: Vec<String> = model.rows().iter().map(|r| r.node.id.clone()).collect();
    let index = model.selected_index();
    let node = model.rows().get(index).map(|r| r.node.clone());
    let select = |model: &mut Model, i: usize| {
        if let Some(id) = ids.get(i) {
            model.selected = Some(id.clone());
            model.last_index = i;
        }
    };
    let last = ids.len().saturating_sub(1);
    match key {
        Key::Char('q' | 'Q') | Key::Esc => return vec![Cmd::Quit],
        Key::Up | Key::Char('k') => select(model, index.saturating_sub(1)),
        Key::Down | Key::Char('j') => select(model, (index + 1).min(last)),
        Key::PageUp => select(model, index.saturating_sub(PAGE)),
        Key::PageDown => select(model, (index + PAGE).min(last)),
        Key::Char(' ') => {
            if let Some(node) = node.filter(|n| !n.children.is_empty())
                && !model.folded.remove(&node.id)
            {
                model.folded.insert(node.id);
            }
        }
        Key::Enter => {
            let target = node.and_then(|n| match n.kind {
                NodeKind::Pane => Some(Target::Pane(n.pane_id)),
                NodeKind::Tab => Some(Target::Tab(n.tab_id)),
                NodeKind::Workspace => Some(Target::Workspace(n.workspace_id)),
                NodeKind::Outside | NodeKind::App => None,
            });
            return target.map(Cmd::Focus).into_iter().collect();
        }
        Key::Char('x' | 'X') => match node {
            Some(n) if n.kind == NodeKind::Pane => {
                model.confirm = Some(Confirm {
                    pane_id: n.pane_id,
                    label: n.label,
                });
            }
            Some(_) => model.message = Some("x closes a pane: select a pane row".into()),
            None => {}
        },
        Key::Char(_) => {}
    }
    vec![]
}

/// Pins the selection to an existing row and scrolls so it stays visible.
fn settle_selection(model: &mut Model) {
    let index = model.selected_index();
    let rows = model.rows();
    let Some(row) = rows.get(index) else {
        model.selected = None;
        model.scroll = 0;
        return;
    };
    let (id, len) = (row.node.id.clone(), rows.len());
    model.selected = Some(id);
    model.last_index = index;
    let visible = model.visible_rows();
    if index < model.scroll {
        model.scroll = index;
    } else if index >= model.scroll + visible {
        model.scroll = index + 1 - visible;
    }
    model.scroll = model.scroll.min(len.saturating_sub(visible));
}
