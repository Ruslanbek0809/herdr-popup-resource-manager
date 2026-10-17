//! The runtime shell: terminal setup, the sampler thread, the event loop, and running `Cmd`s.
//! Everything with logic lives in the library (`app::update`, `view::view`, `tree`, …) and is unit-tested.

use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use herdr_popup_resource_manager::app::{Cmd, Key, Model, Mouse, Msg, update};
use herdr_popup_resource_manager::herdr::client::{Herdr, SocketClient};
use herdr_popup_resource_manager::procs::platform_source;
use herdr_popup_resource_manager::sampler::Sampler;
use herdr_popup_resource_manager::view::view;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;

const SAMPLE_EVERY: Duration = Duration::from_secs(1);
/// The second sample comes sooner so CPU numbers appear quickly.
const FIRST_GAP: Duration = Duration::from_millis(250);
const POLL: Duration = Duration::from_millis(50);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--once") {
        return once();
    }
    let mut terminal = match ratatui::try_init() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot start the terminal UI: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = execute!(std::io::stdout(), EnableMouseCapture).and_then(|_| run(&mut terminal));
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    let restored = ratatui::try_restore();
    match result.and(restored) {
        Ok(()) => ExitCode::SUCCESS,
        // herdr closing the popup removes the terminal; there is nothing to report to.
        Err(_) => ExitCode::FAILURE,
    }
}

/// One sample as JSON (two samples 0.5 s apart, so CPU is filled in). For checks from a shell.
fn once() -> ExitCode {
    let mut sampler = Sampler::new(
        SocketClient::from_env(),
        platform_source(),
        std::process::id(),
    );
    let frame = sampler.sample(Instant::now).and_then(|_| {
        thread::sleep(Duration::from_millis(500));
        sampler.sample(Instant::now)
    });
    match frame {
        Ok(frame) => {
            let json = serde_json::json!({"total": frame.total, "nodes": frame.nodes});
            println!(
                "{}",
                serde_json::to_string_pretty(&json).expect("frame serializes")
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Samples on its own thread so a slow herdr never freezes the keys. Stops when the UI is gone.
fn spawn_sampler(to_ui: Sender<Msg>, requests: Receiver<u64>) {
    thread::spawn(move || {
        let mut sampler = Sampler::new(
            SocketClient::from_env(),
            platform_source(),
            std::process::id(),
        );
        let mut generation = 0;
        let mut wait = FIRST_GAP;
        loop {
            let result = sampler.sample(Instant::now).map_err(|e| e.to_string());
            if to_ui.send(Msg::Sampled { generation, result }).is_err() {
                return;
            }
            match requests.recv_timeout(wait) {
                Ok(g) => generation = g,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            wait = SAMPLE_EVERY;
        }
    });
}

fn to_key(code: KeyCode, modifiers: KeyModifiers) -> Option<Key> {
    Some(match code {
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Key::Esc,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Char(c) => Key::Char(c),
        _ => return None,
    })
}

fn to_mouse(m: MouseEvent) -> Option<Mouse> {
    Some(match m.kind {
        MouseEventKind::Down(MouseButton::Left) => Mouse::Click {
            col: m.column,
            row: m.row,
        },
        MouseEventKind::ScrollUp => Mouse::ScrollUp,
        MouseEventKind::ScrollDown => Mouse::ScrollDown,
        _ => return None,
    })
}

fn total_memory() -> u64 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    system.total_memory()
}

/// Runs messages through `update`, performing the commands it returns. `false` = quit.
fn dispatch(model: &mut Model, msg: Msg, herdr: &mut SocketClient, sampler: &Sender<u64>) -> bool {
    let mut queue = vec![msg];
    while let Some(msg) = queue.pop() {
        for cmd in update(model, msg) {
            match cmd {
                Cmd::Quit => return false,
                Cmd::SampleNow { generation } => {
                    let _ = sampler.send(generation);
                }
                Cmd::Focus(target) => queue.push(Msg::Focused(
                    herdr.focus(&target).map_err(|e| e.to_string()),
                )),
                Cmd::ClosePane(id) => queue.push(Msg::Closed(
                    herdr.close_pane(&id).map_err(|e| e.to_string()),
                )),
            }
        }
    }
    true
}

fn run(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let size = terminal.size()?;
    let cores = thread::available_parallelism().map_or(1, usize::from);
    let mut model = Model::new(total_memory(), cores, (size.width, size.height));
    let (to_ui, from_sampler) = mpsc::channel();
    let (to_sampler, requests) = mpsc::channel();
    spawn_sampler(to_ui, requests);
    let mut herdr = SocketClient::from_env();
    // Draw only after something changed: an idle redraw every poll would cost more than sampling.
    let mut dirty = true;
    loop {
        if dirty {
            terminal.draw(|f| view(&model, f))?;
            dirty = false;
        }
        if event::poll(POLL)? {
            let msg = match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    to_key(k.code, k.modifiers).map(Msg::Key)
                }
                Event::Mouse(m) => to_mouse(m).map(Msg::Mouse),
                Event::Resize(w, h) => Some(Msg::Resized { w, h }),
                _ => None,
            };
            if let Some(msg) = msg {
                if !dispatch(&mut model, msg, &mut herdr, &to_sampler) {
                    return Ok(());
                }
                dirty = true;
            }
        }
        while let Ok(msg) = from_sampler.try_recv() {
            dispatch(&mut model, msg, &mut herdr, &to_sampler);
            dirty = true;
        }
    }
}
