mod app;
mod library;
mod metadata;
mod mouse;
mod player;
mod theme;
mod ui;
mod visualizer;

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::mouse::dispatch_mouse;

type Tui = Terminal<CrosstermBackend<Stdout>>;

fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let stderr_guard = silence_stderr();
    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal, root);
    restore_terminal(&mut terminal)?;
    drop(stderr_guard);
    result
}

/// Redirect fd 2 to /dev/null for the lifetime of the returned guard. Used to
/// swallow ALSA's "underrun occurred" prints (and similar C-library chatter)
/// that would otherwise corrupt the TUI's alternate screen.
#[cfg(unix)]
fn silence_stderr() -> Option<StderrGuard> {
    use std::ffi::CString;
    unsafe {
        let saved = libc::dup(libc::STDERR_FILENO);
        if saved < 0 {
            return None;
        }
        let path = CString::new("/dev/null").ok()?;
        let null = libc::open(path.as_ptr(), libc::O_WRONLY);
        if null < 0 {
            libc::close(saved);
            return None;
        }
        if libc::dup2(null, libc::STDERR_FILENO) < 0 {
            libc::close(null);
            libc::close(saved);
            return None;
        }
        libc::close(null);
        Some(StderrGuard { saved })
    }
}

#[cfg(not(unix))]
fn silence_stderr() -> Option<StderrGuard> { None }

struct StderrGuard {
    #[cfg(unix)]
    saved: libc::c_int,
}

#[cfg(unix)]
impl Drop for StderrGuard {
    fn drop(&mut self) {
        unsafe {
            libc::dup2(self.saved, libc::STDERR_FILENO);
            libc::close(self.saved);
        }
    }
}

fn save_prefs_on_exit(app: &App) {
    theme::save_theme_idx(app.theme_idx);
    visualizer::save_viz_mode(app.viz_mode);
}

fn run(terminal: &mut Tui, root: PathBuf) -> Result<()> {
    let mut app = App::new(root)?;
    let tick = Duration::from_millis(33);

    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        if event::poll(tick)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let viewport = app.hit_map.list_viewport.max(1);
                    if app.theme_open {
                        match key.code {
                            KeyCode::Esc => app.close_theme_picker(false),
                            KeyCode::Enter => app.close_theme_picker(true),
                            KeyCode::Char('t') => app.close_theme_picker(true),
                            KeyCode::Down | KeyCode::Char('j') => app.cycle_theme(1),
                            KeyCode::Up | KeyCode::Char('k') => app.cycle_theme(-1),
                            KeyCode::PageDown => app.cycle_theme(10),
                            KeyCode::PageUp => app.cycle_theme(-10),
                            KeyCode::Home => app.cycle_theme(i32::MIN / 2),
                            KeyCode::End => app.cycle_theme(i32::MAX / 2),
                            _ => {}
                        }
                    } else if app.viz_open {
                        match key.code {
                            KeyCode::Esc => app.close_viz_picker(false),
                            KeyCode::Enter => app.close_viz_picker(true),
                            KeyCode::Char('v') => app.close_viz_picker(true),
                            KeyCode::Down | KeyCode::Char('j') => app.cycle_viz(1),
                            KeyCode::Up | KeyCode::Char('k') => app.cycle_viz(-1),
                            KeyCode::PageDown => app.cycle_viz(10),
                            KeyCode::PageUp => app.cycle_viz(-10),
                            KeyCode::Home => app.cycle_viz(i32::MIN / 2),
                            KeyCode::End => app.cycle_viz(i32::MAX / 2),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                            KeyCode::Char(' ') => app.toggle_play(),
                            KeyCode::Char('v') => app.open_viz_picker(),
                            KeyCode::Char('a') => app.enqueue_selected(),
                            KeyCode::Char('s') => app.toggle_shuffle(),
                            KeyCode::Char('r') => app.toggle_loop(),
                            KeyCode::Char('t') => app.open_theme_picker(),
                            KeyCode::Enter => app.activate_selected(),
                            KeyCode::Backspace => app.ascend_action(),
                            KeyCode::Down | KeyCode::Char('j') => app.move_selection(1, viewport),
                            KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1, viewport),
                            KeyCode::Right | KeyCode::Char('l') => app.descend_selected(),
                            KeyCode::Left | KeyCode::Char('h') => app.ascend_action(),
                            KeyCode::PageDown => app.move_selection(viewport as i32, viewport),
                            KeyCode::PageUp => app.move_selection(-(viewport as i32), viewport),
                            KeyCode::Home => app.move_selection(i32::MIN / 2, viewport),
                            KeyCode::End => app.move_selection(i32::MAX / 2, viewport),
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => dispatch_mouse(&mut app, mouse),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        app.tick();
    }

    save_prefs_on_exit(&app);
    Ok(())
}

fn setup_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
