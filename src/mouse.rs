use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::App;

#[derive(Clone, Copy, Debug)]
pub enum ClickTarget {
    TogglePlay,
    Quit,
    SelectIndex(usize),
    Volume,
    SeekBar,
    ToggleViz,
    ToggleShuffle,
    ToggleLoop,
    OpenThemes,
    ThemeRow(usize),
    VizRow(usize),
}

#[derive(Default)]
pub struct HitMap {
    entries: Vec<(Rect, ClickTarget)>,
    pub list_viewport: usize,
}

impl HitMap {
    pub fn push(&mut self, rect: Rect, target: ClickTarget) {
        self.entries.push((rect, target));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.list_viewport = 0;
    }

    pub fn hit(&self, col: u16, row: u16) -> Option<(Rect, ClickTarget)> {
        self.entries
            .iter()
            .rev()
            .find(|(rect, _)| contains(*rect, col, row))
            .map(|(rect, target)| (*rect, *target))
    }
}

fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

fn fraction_in(rect: Rect, col: u16) -> f32 {
    if rect.width == 0 {
        return 0.0;
    }
    let offset = col.saturating_sub(rect.x) as f32;
    (offset / rect.width as f32).clamp(0.0, 1.0)
}

pub fn dispatch_mouse(app: &mut App, ev: MouseEvent) {
    let hit = app.hit_map.hit(ev.column, ev.row);

    if app.theme_open {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => match hit {
                Some((_, ClickTarget::ThemeRow(i))) => {
                    app.theme_idx = i;
                    app.close_theme_picker(true);
                }
                // Clicking the [themes] button while the picker is open
                // toggles it shut (cancel). Other toolbar buttons still work.
                Some((_, ClickTarget::OpenThemes)) => app.close_theme_picker(false),
                Some((_, t)) => app.handle_click(t),
                None => app.close_theme_picker(false),
            },
            MouseEventKind::Moved => {
                if let Some((_, ClickTarget::ThemeRow(i))) = hit {
                    app.theme_idx = i;
                }
            }
            MouseEventKind::ScrollUp => app.cycle_theme(-1),
            MouseEventKind::ScrollDown => app.cycle_theme(1),
            _ => {}
        }
        return;
    }

    if app.viz_open {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => match hit {
                Some((_, ClickTarget::VizRow(i))) => app.handle_click(ClickTarget::VizRow(i)),
                // Clicking [viz] while the picker is open closes it (cancel).
                Some((_, ClickTarget::ToggleViz)) => app.close_viz_picker(false),
                Some((_, t)) => app.handle_click(t),
                None => app.close_viz_picker(false),
            },
            MouseEventKind::Moved => {
                if let Some((_, ClickTarget::VizRow(i))) = hit {
                    if let Some(mode) = crate::visualizer::VizMode::ALL.get(i).copied() {
                        app.viz_mode = mode;
                    }
                }
            }
            MouseEventKind::ScrollUp => app.cycle_viz(-1),
            MouseEventKind::ScrollDown => app.cycle_viz(1),
            _ => {}
        }
        return;
    }

    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => match hit {
            Some((rect, ClickTarget::SeekBar)) => app.seek_fraction(fraction_in(rect, ev.column)),
            Some((rect, ClickTarget::Volume)) => {
                app.set_volume_fraction(fraction_in(rect, ev.column))
            }
            Some((_, t)) => app.handle_click(t),
            None => {}
        },
        MouseEventKind::Down(MouseButton::Right) => {
            if let Some((_, ClickTarget::SelectIndex(i))) = hit {
                app.enqueue_index(i);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => match hit {
            Some((rect, ClickTarget::SeekBar)) => app.seek_fraction(fraction_in(rect, ev.column)),
            Some((rect, ClickTarget::Volume)) => {
                app.set_volume_fraction(fraction_in(rect, ev.column))
            }
            _ => {}
        },
        MouseEventKind::Moved => {
            let idx = match hit {
                Some((_, ClickTarget::SelectIndex(i))) => Some(i),
                _ => None,
            };
            app.set_hovered(idx);
        }
        MouseEventKind::ScrollUp => match hit {
            Some((_, ClickTarget::Volume)) => app.adjust_volume(1.0),
            _ => app.scroll_by(-3, app.hit_map.list_viewport),
        },
        MouseEventKind::ScrollDown => match hit {
            Some((_, ClickTarget::Volume)) => app.adjust_volume(-1.0),
            _ => app.scroll_by(3, app.hit_map.list_viewport),
        },
        _ => {}
    }
}
