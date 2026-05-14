use std::time::Duration;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use ratatui_image::StatefulImage;

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::library::{self, Entry};
use crate::metadata::TrackMeta;
use crate::mouse::ClickTarget;
use crate::theme::{Theme, THEMES};
use crate::visualizer::{
    DriftPoint, HeartRipple, LightningPoint, Particle, QuakeRing, RadarBlip, TunnelRing, VizMode,
};

static THEME_IDX: AtomicUsize = AtomicUsize::new(0);

fn theme() -> Theme {
    let i = THEME_IDX.load(Ordering::Relaxed);
    let len = THEMES.len();
    THEMES[i.min(len.saturating_sub(1))]
}

#[allow(non_snake_case)]
fn ELECTRIC() -> Color { theme().accent }
#[allow(non_snake_case)]
fn ICE() -> Color { theme().fg2 }
#[allow(non_snake_case)]
fn FROST() -> Color { theme().fg }
#[allow(non_snake_case)]
fn STEEL() -> Color { theme().fg3 }
#[allow(non_snake_case)]
fn DIM_STEEL() -> Color { theme().fg4 }
#[allow(non_snake_case)]
fn VIOLET() -> Color { theme().accent2 }
#[allow(non_snake_case)]
fn DEEP_NAVY() -> Color { theme().bg_sel }
#[allow(non_snake_case)]
fn HOVER_BG() -> Color { theme().bg_hover }
#[allow(non_snake_case)]
fn ALERT() -> Color { theme().alert }

pub fn draw(frame: &mut Frame, app: &mut App) {
    THEME_IDX.store(app.theme_idx, Ordering::Relaxed);
    app.reset_hits();

    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Min(20)])
        .split(chunks[0]);

    // Right column: visualizer when on, otherwise the preview (album art).
    match app.viz_mode {
        VizMode::Off => draw_preview(frame, app, top[1]),
        VizMode::Bars => draw_visualizer(frame, app, top[1]),
        VizMode::Wave => draw_wave(frame, app, top[1]),
        VizMode::Polar => draw_polar(frame, app, top[1]),
        VizMode::Chroma => draw_chroma(frame, app, top[1]),
        VizMode::Spark => draw_spark(frame, app, top[1]),
        VizMode::Drift => draw_drift(frame, app, top[1]),
        VizMode::Aurora => draw_aurora(frame, app, top[1]),
        VizMode::Tunnel => draw_tunnel(frame, app, top[1]),
        VizMode::Radar => draw_radar(frame, app, top[1]),
        VizMode::Mandala => draw_mandala(frame, app, top[1]),
        VizMode::Quake => draw_quake(frame, app, top[1]),
        VizMode::Lightning => draw_lightning(frame, app, top[1]),
        VizMode::Julia => draw_julia(frame, app, top[1]),
        VizMode::Heart => draw_heart(frame, app, top[1]),
        VizMode::Eye => draw_eye(frame, app, top[1]),
    }
    // Left column: theme picker > viz picker > library.
    if app.theme_open {
        draw_theme_picker(frame, app, top[0]);
    } else if app.viz_open {
        draw_viz_picker(frame, app, top[0]);
    } else {
        draw_library(frame, app, top[0]);
    }
    draw_transport(frame, app, chunks[1]);
    draw_status(frame, app, chunks[2]);
}

fn draw_theme_picker(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ELECTRIC()))
        .title(Span::styled(
            format!(" Themes  ({}/{}) ", app.theme_idx + 1, THEMES.len()),
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Reserve last row for a hint.
    let list_h = inner.height.saturating_sub(1) as usize;
    let total = THEMES.len();
    let max_scroll = total.saturating_sub(list_h);
    // Ensure-visible: only bump scroll when the selection moves off-window.
    // Mouse hover sets theme_idx to a row that is already visible, so it
    // never scrolls — only keyboard nav can push selection out of view.
    if app.theme_idx < app.theme_scroll {
        app.theme_scroll = app.theme_idx;
    } else if list_h > 0 && app.theme_idx >= app.theme_scroll + list_h {
        app.theme_scroll = app.theme_idx + 1 - list_h;
    }
    app.theme_scroll = app.theme_scroll.min(max_scroll);
    let start = app.theme_scroll;
    let end = (start + list_h).min(total);

    let label_w: usize = 14;
    for (row, idx) in (start..end).enumerate() {
        let th = THEMES[idx];
        let selected = idx == app.theme_idx;
        let marker = if selected { "▶" } else { " " };
        let name = if th.name.len() > label_w {
            &th.name[..label_w]
        } else {
            th.name
        };
        let pad = label_w.saturating_sub(name.chars().count());
        let row_rect = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        let mut spans: Vec<Span> = Vec::new();
        let name_style = if selected {
            Style::default()
                .fg(th.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.fg2)
        };
        spans.push(Span::styled(format!(" {} ", marker), name_style));
        spans.push(Span::styled(format!("{}{}", name, " ".repeat(pad)), name_style));
        spans.push(Span::styled("  ", Style::default()));
        // Swatch: 5 colored blocks showing key palette colors of this theme.
        let swatch = [th.accent, th.accent2, th.fg2, th.fg3, th.alert];
        for sc in swatch {
            spans.push(Span::styled("██", Style::default().fg(sc)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), row_rect);
        app.register_hit(row_rect, ClickTarget::ThemeRow(idx));
    }

    // Hint line.
    let hint_rect = Rect {
        x: inner.x,
        y: inner.y + inner.height - 1,
        width: inner.width,
        height: 1,
    };
    let key = Style::default().fg(FROST()).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(STEEL());
    let hint = Line::from(vec![
        Span::raw(" "),
        Span::styled("↑↓", key),
        Span::styled(" pick  ", dim),
        Span::styled("⏎", key),
        Span::styled(" save  ", dim),
        Span::styled("esc", key),
        Span::styled(" cancel", dim),
    ]);
    frame.render_widget(Paragraph::new(hint), hint_rect);
}

fn draw_viz_picker(frame: &mut Frame, app: &mut App, area: Rect) {
    let modes = VizMode::ALL;
    let cur = app.viz_mode.index();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ELECTRIC()))
        .title(Span::styled(
            format!(" Visualizer  ({}/{}) ", cur + 1, modes.len()),
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let list_h = inner.height.saturating_sub(1) as usize;
    let total = modes.len();
    let max_scroll = total.saturating_sub(list_h);
    if cur < app.viz_scroll {
        app.viz_scroll = cur;
    } else if list_h > 0 && cur >= app.viz_scroll + list_h {
        app.viz_scroll = cur + 1 - list_h;
    }
    app.viz_scroll = app.viz_scroll.min(max_scroll);
    let start = app.viz_scroll;
    let end = (start + list_h).min(total);

    for (row, idx) in (start..end).enumerate() {
        let mode = modes[idx];
        let selected = idx == cur;
        let marker = if selected { "▶" } else { " " };
        let row_rect = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        let style = if selected {
            Style::default()
                .fg(ELECTRIC())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(ICE())
        };
        let line = Line::from(vec![
            Span::styled(format!(" {} ", marker), style),
            Span::styled(mode.name().to_string(), style),
        ]);
        frame.render_widget(Paragraph::new(line), row_rect);
        app.register_hit(row_rect, ClickTarget::VizRow(idx));
    }

    let hint_rect = Rect {
        x: inner.x,
        y: inner.y + inner.height - 1,
        width: inner.width,
        height: 1,
    };
    let key = Style::default().fg(FROST()).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(STEEL());
    let hint = Line::from(vec![
        Span::raw(" "),
        Span::styled("↑↓", key),
        Span::styled(" pick  ", dim),
        Span::styled("⏎", key),
        Span::styled(" save  ", dim),
        Span::styled("esc", key),
        Span::styled(" cancel", dim),
    ]);
    frame.render_widget(Paragraph::new(hint), hint_rect);
}

fn draw_visualizer(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Bars ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let bins = inner.width as usize;
    let samples = app.tap.snapshot(2048);
    let mags = app.spectrum.analyze(&samples, bins);

    if app.viz_bar_peaks.len() != bins {
        app.viz_bar_peaks = vec![0.0; bins];
    }
    const PEAK_DECAY: f32 = 0.018;
    for (i, m) in mags.iter().enumerate() {
        let p = &mut app.viz_bar_peaks[i];
        if *m > *p {
            *p = *m;
        } else {
            *p = (*p - PEAK_DECAY).max(*m);
        }
    }

    let height = inner.height as usize;
    let half = (height / 2).max(1);

    let mut lines: Vec<Line> = Vec::with_capacity(height);
    for row in 0..height {
        let mut spans: Vec<Span> = Vec::with_capacity(bins);
        for (col, mag) in mags.iter().take(bins).enumerate() {
            let bar_steps = (mag.clamp(0.0, 1.0) * half as f32).round() as usize;
            let peak = app.viz_bar_peaks[col].clamp(0.0, 1.0);
            let peak_steps = (peak * half as f32).round() as usize;

            // Distance from center for this row (1-indexed from the center line).
            let dist = if row < half {
                half - row
            } else {
                row + 1 - half
            };

            let in_bar = dist <= bar_steps && bar_steps > 0;
            let is_peak_cap = peak_steps > bar_steps && dist == peak_steps;

            let (glyph, color) = if in_bar {
                ('█', bar_color(*mag))
            } else if is_peak_cap {
                let cap = if row < half { '▄' } else { '▀' };
                (cap, FROST())
            } else {
                (' ', DIM_STEEL())
            };

            spans.push(Span::styled(glyph.to_string(), Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn bar_color(mag: f32) -> Color {
    grad3(mag.clamp(0.0, 1.0).powf(0.6))
}

/// Three-stop gradient between theme grad_lo → grad_mid → grad_hi at t∈[0,1].
fn grad3(t: f32) -> Color {
    let th = theme();
    let lo = color_to_rgb(th.grad_lo);
    let mid = color_to_rgb(th.grad_mid);
    let hi = color_to_rgb(th.grad_hi);
    if t < 0.5 {
        lerp_rgb(lo, mid, t * 2.0)
    } else {
        lerp_rgb(mid, hi, (t - 0.5) * 2.0)
    }
}

fn color_to_rgb(c: Color) -> [u8; 3] {
    match c {
        Color::Rgb(r, g, b) => [r, g, b],
        _ => [255, 255, 255],
    }
}

/// 5-stop theme palette: fg4 → grad_lo → accent2 → accent → grad_hi.
/// t wraps mod 1 so callers can rotate around it freely.
fn theme_palette(t: f32) -> [u8; 3] {
    let th = theme();
    let stops = [
        color_to_rgb(th.fg4),
        color_to_rgb(th.grad_lo),
        color_to_rgb(th.accent2),
        color_to_rgb(th.accent),
        color_to_rgb(th.grad_hi),
    ];
    let t = t.rem_euclid(1.0);
    let n = (stops.len() - 1) as f32;
    let scaled = t * n;
    let i = (scaled.floor() as usize).min(stops.len() - 2);
    let frac = scaled - i as f32;
    lerp_rgb_u8(stops[i], stops[i + 1], frac)
}

fn lerp_rgb_u8(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

/// Luminance modulation of a theme color: l=0.5 is identity, l<0.5 dims toward
/// black, l>0.5 toward white. Lets visualizers vary "energy" while staying
/// on-palette.
fn brighten(c: [u8; 3], l: f32) -> [u8; 3] {
    let l = l.clamp(0.0, 1.0);
    if l < 0.5 {
        let s = l * 2.0;
        [
            (c[0] as f32 * s) as u8,
            (c[1] as f32 * s) as u8,
            (c[2] as f32 * s) as u8,
        ]
    } else {
        let s = (l - 0.5) * 2.0;
        [
            (c[0] as f32 + (255.0 - c[0] as f32) * s) as u8,
            (c[1] as f32 + (255.0 - c[1] as f32) * s) as u8,
            (c[2] as f32 + (255.0 - c[2] as f32) * s) as u8,
        ]
    }
}

fn draw_polar(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Polar ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;

    // Cells are ~2:4; multiplying x by 2 makes the braille grid roughly square.
    let cell_aspect = 2.0_f32;
    let cx = dx as f32 * 0.5;
    let cy = dy as f32 * 0.5;
    let r_inner = (cy * 0.22).max(2.0);
    let r_outer_max = cy.min(dx as f32 / cell_aspect * 0.5) - 1.0;
    let r_outer_max = r_outer_max.max(r_inner + 2.0);

    // Number of angular slices: enough to cover the outer circumference.
    let circumference = (2.0 * PI_F * r_outer_max) as usize;
    let slices = circumference.max(64).min(256);

    let mags = app.spectrum.analyze(&app.tap.snapshot(2048), slices);

    let mut cells = vec![0u8; cw * ch];
    let mut cell_amp = vec![0.0f32; cw * ch];

    let plot = |cells: &mut [u8], cell_amp: &mut [f32], x: f32, y: f32, amp: f32| {
        if x < 0.0 || y < 0.0 {
            return;
        }
        let xi = x as usize;
        let yi = y as usize;
        if xi >= dx || yi >= dy {
            return;
        }
        let cell_x = xi / 2;
        let cell_y = yi / 4;
        let idx = cell_y * cw + cell_x;
        if idx >= cells.len() {
            return;
        }
        cells[idx] |= dot_bit(xi % 2, yi % 4);
        if amp > cell_amp[idx] {
            cell_amp[idx] = amp;
        }
    };

    // Inner ring (faint).
    let ring_steps = (2.0 * PI_F * r_inner) as usize;
    for s in 0..ring_steps.max(32) {
        let a = s as f32 / ring_steps.max(32) as f32 * 2.0 * PI_F;
        let x = cx + a.cos() * r_inner * cell_aspect;
        let y = cy + a.sin() * r_inner;
        plot(&mut cells, &mut cell_amp, x, y, 0.15);
    }

    // Radial bars.
    let band = r_outer_max - r_inner - 1.0;
    for (i, mag) in mags.iter().enumerate() {
        let angle = (i as f32 / slices as f32) * 2.0 * PI_F - PI_F * 0.5;
        let ca = angle.cos();
        let sa = angle.sin();
        let length = (mag.clamp(0.0, 1.0) * band).max(0.0);
        let steps = (length * 2.0) as usize + 1;
        for s in 0..steps {
            let r = r_inner + (s as f32) * 0.5;
            if r > r_inner + length {
                break;
            }
            let x = cx + ca * r * cell_aspect;
            let y = cy + sa * r;
            plot(&mut cells, &mut cell_amp, x, y, *mag);
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                bar_color(cell_amp[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

const PI_F: f32 = std::f32::consts::PI;

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn draw_chroma(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Chroma ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 12 || inner.height < 3 {
        return;
    }

    let samples = app.tap.snapshot(2048);
    let sr = app.tap.sample_rate();
    let chroma = app.spectrum.analyze_chroma(&samples, sr);

    for i in 0..12 {
        let target = chroma[i].clamp(0.0, 1.0);
        let prev = app.chroma_smoothed[i];
        app.chroma_smoothed[i] = if target > prev {
            prev + (target - prev) * 0.55
        } else {
            prev * 0.86
        };
    }

    let w = inner.width as usize;
    let h = inner.height as usize;
    let bar_h = h.saturating_sub(1);
    let slot_w = (w / 12).max(1);
    let used = slot_w * 12;
    let pad_left = (w - used) / 2;

    const VERT: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇'];

    let mut lines: Vec<Line> = Vec::with_capacity(h);

    for row in 0..bar_h {
        let mut spans: Vec<Span> = Vec::with_capacity(w);
        spans.push(Span::raw(" ".repeat(pad_left)));
        for n in 0..12 {
            let val = app.chroma_smoothed[n].clamp(0.0, 1.0);
            let steps = (val * (bar_h * 8) as f32).round() as usize;
            let full = steps / 8;
            let rem = steps % 8;
            let from_bottom = bar_h - 1 - row;
            let glyph = if from_bottom < full {
                '█'
            } else if from_bottom == full && rem > 0 {
                VERT[rem]
            } else {
                ' '
            };

            // Each pitch class gets a unique stop along the theme palette.
            let hue = n as f32 / 12.0;
            let l = 0.32 + 0.28 * val;
            let color = rgb(brighten(theme_palette(hue), l));

            for _ in 0..slot_w {
                spans.push(Span::styled(
                    glyph.to_string(),
                    Style::default().fg(color),
                ));
            }
        }
        lines.push(Line::from(spans));
    }

    // Label row: each note centered in its slot, dimmed unless that note is hot.
    let mut label_spans: Vec<Span> = Vec::with_capacity(13);
    label_spans.push(Span::raw(" ".repeat(pad_left)));
    for n in 0..12 {
        let val = app.chroma_smoothed[n].clamp(0.0, 1.0);
        let hue = n as f32 / 12.0;
        let l = 0.45 + 0.30 * val;
        let color = rgb(brighten(theme_palette(hue), l));
        let mut style = Style::default().fg(color);
        if val > 0.7 {
            style = style.add_modifier(Modifier::BOLD);
        }
        let label = center_in(NOTE_NAMES[n], slot_w);
        label_spans.push(Span::styled(label, style));
    }
    lines.push(Line::from(label_spans));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn center_in(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.chars().take(width).collect();
    }
    let total_pad = width - len;
    let left = total_pad / 2;
    let right = total_pad - left;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
}

fn draw_spark(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Spark ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;

    // VU-style trigger: max |sample| over a recent window, with a decaying peak-hold
    // acting as the gate. A fresh attack must clearly exceed the hold to spawn.
    let amp_samples = app.tap.snapshot(1024);
    let mut inst_peak = 0.0f32;
    for s in &amp_samples {
        let a = s.abs();
        if a > inst_peak {
            inst_peak = a;
        }
    }

    // Spectrum still drives hue + background twinkles.
    let bins = 24;
    let spec_samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&spec_samples, bins);
    let mut bass = 0.0f32;
    let mut mid = 0.0f32;
    let mut treble = 0.0f32;
    for i in 0..bins {
        if i < bins / 3 {
            bass += spec[i];
        } else if i < 2 * bins / 3 {
            mid += spec[i];
        } else {
            treble += spec[i];
        }
    }
    // Dominant band picks a position in the theme palette: bass = deep
    // (grad_lo area), mid = accent2, treble = bright (toward grad_hi).
    // Particles stay in a tight band around it so the burst reads as one
    // color family from the active theme.
    let dominant_hue = if bass >= mid && bass >= treble {
        0.25
    } else if treble > mid {
        0.85
    } else {
        0.55
    };

    let prev_hold = app.spark_peak_hold;
    let is_onset = inst_peak > 0.18 && inst_peak > prev_hold + 0.06;
    if is_onset {
        app.spark_peak_hold = inst_peak;
        let cx = dx as f32 / 2.0;
        let cy = dy as f32 / 2.0;
        let surplus = (inst_peak - prev_hold).clamp(0.0, 1.0);
        let n_particles = (50 + (inst_peak * 90.0 + surplus * 120.0) as usize).min(220);
        let burst_speed = 1.4 + inst_peak * 2.4;
        for _ in 0..n_particles {
            let angle = rand_f32(&mut app.spark_rng) * 2.0 * PI_F;
            let speed = burst_speed * (0.4 + rand_f32(&mut app.spark_rng) * 1.8);
            // Narrow jitter around the band's theme stop.
            let hue = (dominant_hue + (rand_f32(&mut app.spark_rng) - 0.5) * 0.14)
                .rem_euclid(1.0);
            app.spark_particles.push(Particle {
                x: cx,
                y: cy,
                vx: angle.cos() * speed * 2.0,
                vy: angle.sin() * speed,
                life: 1.0,
                hue,
            });
        }
    } else {
        // Slow decay so loud sustained passages don't keep firing each frame.
        app.spark_peak_hold = (prev_hold * 0.94).max(inst_peak);
    }

    // Update particles. No gravity — pure radial drift with slight slowdown.
    for p in &mut app.spark_particles {
        p.x += p.vx;
        p.y += p.vy;
        p.vx *= 0.985;
        p.vy *= 0.985;
        p.life -= 0.018;
    }
    let max_x = dx as f32;
    let max_y = dy as f32;
    app.spark_particles.retain(|p| {
        p.life > 0.0 && p.x > -2.0 && p.y > -2.0 && p.x < max_x + 2.0 && p.y < max_y + 2.0
    });
    if app.spark_particles.len() > 1200 {
        let drop = app.spark_particles.len() - 1200;
        app.spark_particles.drain(0..drop);
    }

    let mut cells = vec![0u8; cw * ch];
    let mut cell_color = vec![[0u8; 3]; cw * ch];

    // Ambient twinkle layer: faint points whose density scales with overall energy.
    let energy = spec.iter().sum::<f32>() / bins as f32;
    let twinkles = (energy * 80.0) as usize;
    for _ in 0..twinkles {
        let xi = (rand_f32(&mut app.spark_rng) * dx as f32) as usize;
        let yi = (rand_f32(&mut app.spark_rng) * dy as f32) as usize;
        if xi >= dx || yi >= dy {
            continue;
        }
        let idx = (yi / 4) * cw + (xi / 2);
        cells[idx] |= dot_bit(xi % 2, yi % 4);
        if cell_color[idx] == [0, 0, 0] {
            // Ambient twinkle picks the theme's mid-bright stop, dimmed.
            cell_color[idx] = brighten(theme_palette(0.55), 0.30 + energy * 0.20);
        }
    }

    for p in &app.spark_particles {
        if p.x < 0.0 || p.y < 0.0 {
            continue;
        }
        let xi = p.x as usize;
        let yi = p.y as usize;
        if xi >= dx || yi >= dy {
            continue;
        }
        let idx = (yi / 4) * cw + (xi / 2);
        cells[idx] |= dot_bit(xi % 2, yi % 4);
        let l = 0.30 + 0.55 * p.life;
        let col = brighten(theme_palette(p.hue), l);
        // Brightest wins to keep colors crisp under overlap.
        if luma(col) > luma(cell_color[idx]) {
            cell_color[idx] = col;
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                rgb(cell_color[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_drift(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Drift ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;

    let bins = 64;
    let samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&samples, bins);

    let mut sum = 0.0f32;
    let mut wsum = 0.0f32;
    let mut bass = 0.0f32;
    let mut treble = 0.0f32;
    for (i, m) in spec.iter().enumerate() {
        sum += m;
        wsum += i as f32 * m;
        if i < bins / 3 {
            bass += m;
        } else if i >= 2 * bins / 3 {
            treble += m;
        }
    }
    let centroid = if sum > 0.001 {
        (wsum / sum) / (bins - 1) as f32
    } else {
        0.5
    };
    let energy = (sum / bins as f32).clamp(0.0, 1.0).powf(0.65);
    let bass_n = (bass * 3.0 / bins as f32).clamp(0.0, 1.0);
    let treble_n = (treble * 3.0 / bins as f32).clamp(0.0, 1.0);

    // Init position once.
    if app.drift_x == 0.0 && app.drift_y == 0.0 {
        app.drift_x = dx as f32 * 0.5;
        app.drift_y = dy as f32 * 0.5;
    }

    // Spring target derived from spectrum, plus a swirling Lissajous offset.
    let target_x = centroid.clamp(0.0, 1.0) * (dx - 1) as f32;
    let target_y = (1.0 - energy) * (dy - 1) as f32;

    let phase = app.drift_phase * 2.0 * PI_F;
    let swirl_x = phase.cos() * (dx as f32 * 0.20) * energy;
    let swirl_y = (phase * 1.37).sin() * (dy as f32 * 0.28) * energy;

    let want_x = (target_x + swirl_x).clamp(0.0, (dx - 1) as f32);
    let want_y = (target_y + swirl_y).clamp(0.0, (dy - 1) as f32);

    // Bass and treble jolt the velocity directly so beats fling the comet.
    let kick = 1.5 * (bass_n.powf(2.0));
    let zip = 1.0 * (treble_n.powf(2.0));
    let kick_angle = rand_f32(&mut app.spark_rng) * 2.0 * PI_F;

    let stiffness = 0.12;
    let drag = 0.88;
    app.drift_vx = app.drift_vx * drag
        + (want_x - app.drift_x) * stiffness
        + kick_angle.cos() * kick * 1.6
        + (phase * 2.1).sin() * zip * 1.1;
    app.drift_vy = app.drift_vy * drag
        + (want_y - app.drift_y) * stiffness
        + kick_angle.sin() * kick * 1.0
        + (phase * 2.7).cos() * zip * 0.9;

    // Speed clamp — calmer top end so motion reads as flowing, not flailing.
    let max_v = (dx.min(dy) as f32) * 0.12;
    let v_mag = (app.drift_vx * app.drift_vx + app.drift_vy * app.drift_vy).sqrt();
    if v_mag > max_v {
        let s = max_v / v_mag;
        app.drift_vx *= s;
        app.drift_vy *= s;
    }

    app.drift_x = (app.drift_x + app.drift_vx).clamp(0.0, (dx - 1) as f32);
    app.drift_y = (app.drift_y + app.drift_vy).clamp(0.0, (dy - 1) as f32);
    if app.drift_x <= 0.0 || app.drift_x >= (dx - 1) as f32 {
        app.drift_vx *= -0.7;
    }
    if app.drift_y <= 0.0 || app.drift_y >= (dy - 1) as f32 {
        app.drift_vy *= -0.7;
    }

    // Radius grows with current energy — louder = fatter comet.
    let radius = 0.5 + energy * 3.6;

    app.drift_trail.push_back(DriftPoint {
        x: app.drift_x,
        y: app.drift_y,
        r: radius,
    });
    while app.drift_trail.len() > 140 {
        app.drift_trail.pop_front();
    }
    app.drift_phase = (app.drift_phase + 0.010 + energy * 0.008).rem_euclid(1.0);

    let mut cells = vec![0u8; cw * ch];
    let mut cell_color = vec![[0u8; 3]; cw * ch];

    let len = app.drift_trail.len().max(1);
    let trail: Vec<DriftPoint> = app.drift_trail.iter().copied().collect();

    let plot = |cells: &mut [u8], cell_color: &mut [[u8; 3]], xi: i32, yi: i32, col: [u8; 3]| {
        if xi < 0 || yi < 0 || xi >= dx as i32 || yi >= dy as i32 {
            return;
        }
        let xu = xi as usize;
        let yu = yi as usize;
        let idx = (yu / 4) * cw + (xu / 2);
        cells[idx] |= dot_bit(xu % 2, yu % 4);
        if luma(col) > luma(cell_color[idx]) {
            cell_color[idx] = col;
        }
    };
    let plot_disc =
        |cells: &mut [u8], cell_color: &mut [[u8; 3]], cx: i32, cy: i32, r: f32, col: [u8; 3]| {
            let ri = r.max(0.0).ceil() as i32;
            if ri <= 0 {
                plot(cells, cell_color, cx, cy, col);
                return;
            }
            let r2 = r * r;
            for oy in -ri..=ri {
                for ox in -ri..=ri {
                    let d2 = (ox * ox + oy * oy) as f32;
                    if d2 <= r2 {
                        plot(cells, cell_color, cx + ox, cy + oy, col);
                    }
                }
            }
        };

    for i in 0..trail.len() {
        let age = i as f32 / len as f32; // 0 = oldest, 1 = newest
        let hue = (age * 0.85 + app.drift_phase).rem_euclid(1.0);
        let l = 0.20 + 0.55 * age;
        let col = brighten(theme_palette(hue), l);

        let p = trail[i];
        // Taper older parts of the trail so the comet has a head and a tail.
        let r_here = p.r * (0.35 + 0.65 * age);

        if i == 0 {
            plot_disc(&mut cells, &mut cell_color, p.x.round() as i32, p.y.round() as i32, r_here, col);
            continue;
        }
        let prev = trail[i - 1];
        let prev_age = (i - 1) as f32 / len as f32;
        let r_prev = prev.r * (0.35 + 0.65 * prev_age);

        let dxs = p.x - prev.x;
        let dys = p.y - prev.y;
        let steps = dxs.abs().max(dys.abs()).round().max(1.0) as i32;
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let ix = (prev.x + dxs * t).round() as i32;
            let iy = (prev.y + dys * t).round() as i32;
            let ir = r_prev + (r_here - r_prev) * t;
            plot_disc(&mut cells, &mut cell_color, ix, iy, ir, col);
        }
    }

    // Bright head highlight.
    if let Some(&head) = trail.last() {
        let hx = head.x.round() as i32;
        let hy = head.y.round() as i32;
        let head_col = brighten(theme_palette(app.drift_phase), 0.92);
        plot_disc(&mut cells, &mut cell_color, hx, hy, (head.r * 0.6).max(1.0), head_col);
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                rgb(cell_color[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn rgb(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

fn luma(c: [u8; 3]) -> u32 {
    // Rough brightness used to keep brightest contributor when cells overlap.
    c[0] as u32 * 2 + c[1] as u32 * 3 + c[2] as u32
}

fn xorshift_u64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x.max(1);
    *state
}

fn rand_f32(state: &mut u64) -> f32 {
    let r = (xorshift_u64(state) >> 32) as u32;
    r as f32 / u32::MAX as f32
}

fn draw_wave(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Wave ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2; // braille subcolumns
    let dy = ch * 4; // braille subrows

    let samples = app.tap.snapshot(dx.max(64));

    let mut cells = vec![0u8; cw * ch];
    let mut cell_amp = vec![0.0f32; cw * ch];

    if !samples.is_empty() {
        let half_y = dy as f32 * 0.5;
        let n = samples.len();
        let mut prev: Option<i32> = None;
        for sx in 0..dx {
            let s_idx = (sx * n) / dx;
            let s = samples[s_idx].clamp(-1.0, 1.0);
            let y_f = half_y - s * (half_y - 1.0);
            let y = (y_f.round() as i32).clamp(0, dy as i32 - 1);

            let (lo, hi) = match prev {
                Some(p) => (p.min(y), p.max(y)),
                None => (y, y),
            };

            for yi in lo..=hi {
                let cell_x = sx / 2;
                let cell_y = (yi as usize) / 4;
                let dot_col = sx % 2;
                let dot_row = (yi as usize) % 4;
                let idx = cell_y * cw + cell_x;
                cells[idx] |= dot_bit(dot_col, dot_row);
                let amp = s.abs();
                if amp > cell_amp[idx] {
                    cell_amp[idx] = amp;
                }
            }
            prev = Some(y);
        }
    } else {
        // No audio yet — draw a faint flatline through the centre.
        let mid_dot = dy / 2;
        for sx in 0..dx {
            let cell_x = sx / 2;
            let cell_y = mid_dot / 4;
            let idx = cell_y * cw + cell_x;
            cells[idx] |= dot_bit(sx % 2, mid_dot % 4);
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                wave_color(cell_amp[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn dot_bit(col: usize, row: usize) -> u8 {
    match (col, row) {
        (0, 0) => 1 << 0,
        (0, 1) => 1 << 1,
        (0, 2) => 1 << 2,
        (1, 0) => 1 << 3,
        (1, 1) => 1 << 4,
        (1, 2) => 1 << 5,
        (0, 3) => 1 << 6,
        (1, 3) => 1 << 7,
        _ => 0,
    }
}

fn wave_color(amp: f32) -> Color {
    grad3(amp.clamp(0.0, 1.0).powf(0.55))
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::Rgb(
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    )
}


fn draw_preview(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Preview ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let Some(target) = app.preview_target() else {
        return;
    };

    let art_h = (inner.height as i32 - 8).max(0) as u16;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(art_h), Constraint::Min(0)])
        .split(inner);

    if let Some(proto) = app.image_cache.get_mut(&target.art_key) {
        if art_h > 0 {
            frame.render_stateful_widget(StatefulImage::default(), layout[0], proto);
        }
    } else if art_h > 0 {
        let placeholder = Paragraph::new(Line::from(Span::styled(
            "♪ no album art",
            Style::default().fg(DIM_STEEL()),
        )));
        frame.render_widget(placeholder, layout[0]);
    }

    let mut lines: Vec<Line> = Vec::new();
    if let Some(track) = target.track.as_ref() {
        let meta = match app.meta_cache.get(track) {
            Some(m) => m.clone(),
            None => TrackMeta::default(),
        };
        let filename = track
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)")
            .to_string();
        lines.push(Line::from(Span::styled(
            meta.title.clone().unwrap_or(filename),
            Style::default().fg(FROST()).add_modifier(Modifier::BOLD),
        )));
        if let Some(artist) = meta.artist.as_ref() {
            lines.push(Line::from(Span::styled(
                artist.clone(),
                Style::default().fg(ELECTRIC()),
            )));
        }
        let album_line = match (meta.album.as_ref(), meta.year) {
            (Some(a), Some(y)) => Some(format!("{}  ·  {}", a, y)),
            (Some(a), None) => Some(a.clone()),
            (None, Some(y)) => Some(y.to_string()),
            _ => None,
        };
        if let Some(s) = album_line {
            lines.push(Line::from(Span::styled(s, Style::default().fg(ICE()))));
        }
        if let Some(g) = meta.genre.as_ref() {
            lines.push(Line::from(Span::styled(
                g.clone(),
                Style::default().fg(STEEL()),
            )));
        }
        if let Some(secs) = meta.duration_secs {
            let m = secs / 60;
            let s = secs % 60;
            lines.push(Line::from(Span::styled(
                format!("{:02}:{:02}", m, s),
                Style::default().fg(STEEL()),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            format!("{}/", target.fallback_label),
            Style::default().fg(FROST()).add_modifier(Modifier::BOLD),
        )));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, layout[1]);
}

fn draw_library(frame: &mut Frame, app: &mut App, area: Rect) {
    let root_label = app
        .root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".");
    let rel = app
        .cwd
        .strip_prefix(&app.root)
        .ok()
        .and_then(|p| if p.as_os_str().is_empty() { None } else { Some(p) });
    let crumb = match rel {
        Some(r) => format!(" {}/{} ", root_label, r.display()),
        None => format!(" {} ", root_label),
    };
    let count = app
        .entries
        .iter()
        .filter(|e| matches!(e, Entry::Track(_)))
        .count();
    let title = format!("{}· {} tracks ", crumb, count);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            title,
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.entries.is_empty() {
        let para = Paragraph::new(Span::styled(
            "Empty folder.",
            Style::default().fg(DIM_STEEL()),
        ));
        frame.render_widget(para, inner);
        return;
    }

    let viewport = inner.height as usize;
    app.hit_map.list_viewport = viewport;
    // Clamp scroll so a resized window doesn't leave a gap below the last row,
    // but DO NOT force `selected` back into view — that fights with mouse scroll.
    let max_scroll = app.entries.len().saturating_sub(viewport.max(1));
    if app.scroll > max_scroll {
        app.scroll = max_scroll;
    }

    let start = app.scroll;
    let end = (start + viewport).min(app.entries.len());
    let now_playing = app.now_playing.clone();
    let selected = app.selected;
    let hovered = app.hovered;

    let mut lines: Vec<Line> = Vec::with_capacity(end - start);
    for (offset, idx) in (start..end).enumerate() {
        let entry = &app.entries[idx];
        let label = library::entry_label(entry);
        let is_selected = idx == selected;
        let is_hovered = hovered == Some(idx);
        let is_playing = matches!(entry, Entry::Track(p) if Some(p) == now_playing.as_ref());

        let queue_pos = match entry {
            Entry::Track(p) => app.queue_position(p),
            _ => None,
        };
        let (glyph, base_fg): (String, Color) = match entry {
            Entry::Parent => ("↑ ".to_string(), STEEL()),
            Entry::Folder(_) => ("▸ ".to_string(), ICE()),
            Entry::Track(_) => {
                if is_playing {
                    ("▶ ".to_string(), ELECTRIC())
                } else if let Some(pos) = queue_pos {
                    let n = pos + 1;
                    let g = if n < 10 {
                        format!("{} ", n)
                    } else {
                        "·•".to_string()
                    };
                    (g, VIOLET())
                } else {
                    ("  ".to_string(), FROST())
                }
            }
        };

        let mut style = Style::default().fg(base_fg);
        if is_playing {
            style = style.add_modifier(Modifier::BOLD);
        }
        if is_selected {
            style = style.bg(DEEP_NAVY()).fg(ELECTRIC()).add_modifier(Modifier::BOLD);
        } else if is_hovered {
            style = style.bg(HOVER_BG());
        }

        let row_width = inner.width as usize;
        let used = glyph.chars().count() + label.chars().count();
        let pad = row_width.saturating_sub(used);
        lines.push(Line::from(vec![
            Span::styled(glyph, style),
            Span::styled(label, style),
            Span::styled(" ".repeat(pad), style),
        ]));

        let row_rect = Rect {
            x: inner.x,
            y: inner.y + offset as u16,
            width: inner.width,
            height: 1,
        };
        app.register_hit(row_rect, ClickTarget::SelectIndex(idx));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}

fn draw_transport(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(12),
            Constraint::Min(0),
            Constraint::Length(16),
            Constraint::Length(20),
        ])
        .split(area);

    let label = if app.player.is_playing() {
        " ⏸  Pause "
    } else {
        " ▶  Play  "
    };
    let button = Paragraph::new(Line::from(Span::styled(
        label,
        Style::default()
            .fg(DEEP_NAVY())
            .bg(ELECTRIC())
            .add_modifier(Modifier::BOLD),
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(STEEL())),
    );
    frame.render_widget(button, chunks[0]);
    app.register_hit(chunks[0], ClickTarget::TogglePlay);

    let ratio = match app.track_duration {
        Some(total) if total.as_secs_f64() > 0.0 => {
            (app.elapsed.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    let bar_title = app
        .now_playing
        .as_ref()
        .map(|track| {
            let title = app
                .meta_cache
                .get(track)
                .and_then(|m| m.title.clone())
                .or_else(|| {
                    track
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "(unknown)".to_string());
            format!(" {} ", title)
        })
        .unwrap_or_else(|| " Progress ".to_string());
    let progress_inner = draw_bar(
        frame,
        chunks[1],
        &bar_title,
        ratio,
        ELECTRIC(),
        '▒',
        '░',
    );
    // hit zone spans full transport height so clicks on the borders count too,
    // but x/width track the inner bar so fractions stay accurate.
    let seek_hit = Rect {
        x: progress_inner.x,
        y: chunks[1].y,
        width: progress_inner.width,
        height: chunks[1].height,
    };
    app.register_hit(seek_hit, ClickTarget::SeekBar);

    let pos = Paragraph::new(Line::from(Span::styled(
        format_position(app),
        Style::default().fg(ICE()),
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(STEEL())),
    );
    frame.render_widget(pos, chunks[2]);

    let vol = app.player.volume();
    let vol_pct = (vol * 100.0).round() as u16;
    let title = format!(" Vol {:>3}% ", vol_pct);
    let vol_inner = draw_smooth_bar(frame, chunks[3], &title, vol as f64, VIOLET());
    let vol_hit = Rect {
        x: vol_inner.x,
        y: chunks[3].y,
        width: vol_inner.width,
        height: chunks[3].height,
    };
    app.register_hit(vol_hit, ClickTarget::Volume);
}

fn draw_smooth_bar(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    ratio: f64,
    fill: Color,
) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            title.to_string(),
            Style::default().fg(ICE()),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return inner;
    }

    // Sub-cell resolution via eighth-block characters: 8x the cells.
    const EIGHTHS: [char; 9] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    let width = inner.width as usize;
    let ratio = ratio.clamp(0.0, 1.0);
    let total_eighths = (ratio * (width * 8) as f64).round() as usize;
    let full_cells = total_eighths / 8;
    let remainder = total_eighths % 8;

    let mut buf = String::with_capacity(width * 3);
    for _ in 0..full_cells {
        buf.push('█');
    }
    let mut consumed = full_cells;
    if remainder > 0 && full_cells < width {
        buf.push(EIGHTHS[remainder]);
        consumed += 1;
    }
    let empty_count = width.saturating_sub(consumed);
    let empty: String = "░".repeat(empty_count);

    let fill_style = Style::default().fg(fill).add_modifier(Modifier::BOLD);
    let empty_style = Style::default().fg(DIM_STEEL());
    let line = Line::from(vec![
        Span::styled(buf, fill_style),
        Span::styled(empty, empty_style),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
    inner
}

fn draw_bar(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    ratio: f64,
    fill: Color,
    fill_char: char,
    empty_char: char,
) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            title.to_string(),
            Style::default().fg(ICE()),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return inner;
    }

    let width = inner.width as usize;
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = ((ratio * width as f64).round() as usize).min(width);
    let empty_count = width - filled;

    let empty_style = Style::default().fg(DIM_STEEL());
    let fill_style = Style::default().fg(fill).add_modifier(Modifier::BOLD);

    let line = Line::from(vec![
        Span::styled(fill_char.to_string().repeat(filled), fill_style),
        Span::styled(empty_char.to_string().repeat(empty_count), empty_style),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
    inner
}

fn draw_status(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(11),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(8),
        ])
        .split(area);

    let key = Style::default().fg(FROST()).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(STEEL());
    let mut spans = vec![
        Span::raw(" "),
        Span::styled("↑↓", key),
        Span::styled(" move  ", dim),
        Span::styled("⏎", key),
        Span::styled(" open  ", dim),
        Span::styled("⌫", key),
        Span::styled(" up  ", dim),
        Span::styled("space", key),
        Span::styled(" pause  ", dim),
        Span::styled("a", key),
        Span::styled(" queue  ", dim),
        Span::styled("s", key),
        Span::styled(" shuf  ", dim),
        Span::styled("r", key),
        Span::styled(" loop  ", dim),
        Span::styled("v", key),
        Span::styled(" viz", dim),
    ];
    if !app.queue.is_empty() {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            format!("queue: {}", app.queue.len()),
            Style::default().fg(VIOLET()).add_modifier(Modifier::BOLD),
        ));
    }
    if !app.status.is_empty() {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            app.status.clone(),
            Style::default().fg(ALERT()),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), chunks[0]);

    let shuf_style = if app.shuffle {
        Style::default()
            .fg(DEEP_NAVY())
            .bg(ELECTRIC())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(VIOLET())
    };
    let shuf = Paragraph::new(Line::from(Span::styled(" [shuffle] ", shuf_style)));
    frame.render_widget(shuf, chunks[1]);
    app.register_hit(chunks[1], ClickTarget::ToggleShuffle);

    let loop_style = if app.loop_one {
        Style::default()
            .fg(DEEP_NAVY())
            .bg(ELECTRIC())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(VIOLET())
    };
    let loop_btn = Paragraph::new(Line::from(Span::styled(" [loop] ", loop_style)));
    frame.render_widget(loop_btn, chunks[2]);
    app.register_hit(chunks[2], ClickTarget::ToggleLoop);

    let viz_style = if app.viz_open || app.viz_mode.is_on() {
        Style::default()
            .fg(DEEP_NAVY())
            .bg(ELECTRIC())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(VIOLET())
    };
    let viz = Paragraph::new(Line::from(Span::styled(app.viz_mode.label(), viz_style)));
    frame.render_widget(viz, chunks[3]);
    app.register_hit(chunks[3], ClickTarget::ToggleViz);

    let theme_style = if app.theme_open {
        Style::default()
            .fg(DEEP_NAVY())
            .bg(ELECTRIC())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(VIOLET())
    };
    let themes = Paragraph::new(Line::from(Span::styled(" [themes] ", theme_style)));
    frame.render_widget(themes, chunks[4]);
    app.register_hit(chunks[4], ClickTarget::OpenThemes);

    let quit = Paragraph::new(Line::from(Span::styled(
        " [quit] ",
        Style::default().fg(ALERT()).add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(quit, chunks[5]);
    app.register_hit(chunks[5], ClickTarget::Quit);
}

fn format_position(app: &App) -> String {
    let elapsed = fmt_dur(app.elapsed);
    match app.track_duration {
        Some(total) => format!(" {} / {} ", elapsed, fmt_dur(total)),
        None => format!(" {} / --:-- ", elapsed),
    }
}

fn fmt_dur(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn add_color(existing: [u8; 3], add: [u8; 3]) -> [u8; 3] {
    [
        (existing[0] as u32 + add[0] as u32).min(255) as u8,
        (existing[1] as u32 + add[1] as u32).min(255) as u8,
        (existing[2] as u32 + add[2] as u32).min(255) as u8,
    ]
}

fn draw_aurora(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Aurora ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;

    // 4 bands: sub, bass, mid, treble. Hues stay in cyan/violet/teal range.
    let bins = 32;
    let samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&samples, bins);

    let mut raw = [0.0f32; 4];
    let split = [
        0..(bins / 8),
        (bins / 8)..(bins / 3),
        (bins / 3)..(2 * bins / 3),
        (2 * bins / 3)..bins,
    ];
    for (b, range) in split.iter().enumerate() {
        let len = range.len().max(1);
        let sum: f32 = spec[range.clone()].iter().copied().sum();
        raw[b] = (sum / len as f32).clamp(0.0, 1.0);
    }
    // Asymmetric smoothing: fast attack, slow release. Aurora "breathes".
    for i in 0..4 {
        let target = raw[i];
        let prev = app.aurora_bands[i];
        app.aurora_bands[i] = if target > prev {
            prev + (target - prev) * 0.45
        } else {
            prev + (target - prev) * 0.08
        };
    }
    app.aurora_phase = (app.aurora_phase + 0.025).rem_euclid(TAU_F);

    // Ribbon hues — held in cyan/blue/violet/mint to keep the theme.
    // 4 ribbons spread evenly along the theme palette: deep → bright.
    const RIBBON_HUES: [f32; 4] = [0.15, 0.40, 0.65, 0.90];

    let mut cell_color = vec![[0u8; 3]; cw * ch];
    let mut cells = vec![0u8; cw * ch];

    let band_height = dy as f32 / 4.0;
    let half_band = band_height * 0.5;

    for (b, &hue) in RIBBON_HUES.iter().enumerate() {
        let energy = app.aurora_bands[b];
        let base_y = (b as f32 + 0.5) * band_height;
        // Amplitude scales with energy; ribbon also swells thicker on energy.
        let amplitude = (half_band * 0.85) * (0.15 + energy);
        let thickness = (1.0 + energy * 5.0) as i32;

        for sx in 0..dx {
            let x = sx as f32;
            let phase = app.aurora_phase * (1.0 + b as f32 * 0.13)
                + x * 0.07
                + b as f32 * 1.7;
            let wobble = phase.sin() + (phase * 0.41 + b as f32).sin() * 0.45;
            let y_center = base_y + wobble * amplitude;

            // Glow profile across thickness: brightest at center, fading outward.
            for t_off in -thickness..=thickness {
                let yi = y_center as i32 + t_off;
                if yi < 0 || yi >= dy as i32 {
                    continue;
                }
                let dist = (t_off.abs() as f32) / (thickness as f32 + 0.1);
                let glow = (1.0 - dist).powf(1.6);
                let l = (0.18 + energy * 0.55) * glow;
                if l < 0.02 {
                    continue;
                }
                let col = brighten(theme_palette(hue), l.clamp(0.0, 0.85));

                let cell_x = sx / 2;
                let cell_y = (yi as usize) / 4;
                let idx = cell_y * cw + cell_x;
                cells[idx] |= dot_bit(sx % 2, (yi as usize) % 4);
                // Additive blending so overlapping ribbons brighten.
                cell_color[idx] = add_color(cell_color[idx], col);
            }
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                rgb(cell_color[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_tunnel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Tunnel ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 6 || inner.height < 6 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;
    let aspect = 2.0_f32;
    let cx = dx as f32 / 2.0;
    let cy = dy as f32 / 2.0;
    let max_r = (dy as f32 * 0.5).min(dx as f32 / aspect * 0.5) + 4.0;

    let bins = 24;
    let samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&samples, bins);
    let mut bass = 0.0f32;
    let mut mid = 0.0f32;
    let mut treble = 0.0f32;
    for i in 0..bins {
        if i < bins / 3 {
            bass += spec[i];
        } else if i < 2 * bins / 3 {
            mid += spec[i];
        } else {
            treble += spec[i];
        }
    }
    bass /= (bins / 3) as f32;
    mid /= (bins / 3) as f32;
    treble /= (bins - 2 * bins / 3) as f32;

    // VU-style peak-hold trigger feeds extra rings on the beat.
    let amp_samples = app.tap.snapshot(1024);
    let mut inst_peak = 0.0f32;
    for s in &amp_samples {
        let a = s.abs();
        if a > inst_peak {
            inst_peak = a;
        }
    }
    let prev_hold = app.tunnel_peak_hold;
    let beat = inst_peak > 0.18 && inst_peak > prev_hold + 0.06;
    if beat {
        app.tunnel_peak_hold = inst_peak;
    } else {
        app.tunnel_peak_hold = (prev_hold * 0.94).max(inst_peak);
    }

    // Time-based spawn cadence, accelerated by bass.
    app.tunnel_spawn_timer += 1.0 + bass * 3.5;
    let spawn_interval = 6.0;
    while app.tunnel_spawn_timer >= spawn_interval {
        app.tunnel_spawn_timer -= spawn_interval;
        // Ring hue = position along the theme palette: bass = deep, mid =
        // accent2 area, treble = bright. Beat flashes use the bright end.
        let hue = if bass >= mid && bass >= treble {
            0.25
        } else if treble > mid {
            0.85
        } else {
            0.55
        };
        let bright = (0.4 + bass * 0.6).clamp(0.3, 1.0);
        app.tunnel_rings.push(TunnelRing {
            radius: 1.5,
            hue,
            brightness: bright,
        });
    }
    if beat {
        // Extra burst on the beat — a flash of inner rings at the bright stop.
        for i in 0..3 {
            app.tunnel_rings.push(TunnelRing {
                radius: 1.0 + i as f32 * 0.6,
                hue: 0.90,
                brightness: 0.95,
            });
        }
    }

    // Advance rings — bass speeds up the rush.
    let speed = 0.55 + bass * 1.4 + treble * 0.4;
    for r in app.tunnel_rings.iter_mut() {
        r.radius += speed;
    }
    app.tunnel_rings.retain(|r| r.radius < max_r);

    let mut cells = vec![0u8; cw * ch];
    let mut cell_color = vec![[0u8; 3]; cw * ch];

    let plot = |cells: &mut [u8], cell_color: &mut [[u8; 3]], xi: i32, yi: i32, col: [u8; 3]| {
        if xi < 0 || yi < 0 || xi >= dx as i32 || yi >= dy as i32 {
            return;
        }
        let xu = xi as usize;
        let yu = yi as usize;
        let idx = (yu / 4) * cw + (xu / 2);
        cells[idx] |= dot_bit(xu % 2, yu % 4);
        cell_color[idx] = add_color(cell_color[idx], col);
    };

    // Draw each ring as a circle in braille. Brightness fades as ring grows.
    for ring in &app.tunnel_rings {
        let depth = (ring.radius / max_r).clamp(0.0, 1.0);
        // Rings far from center fade out (perspective).
        let fade = (1.0 - depth).powf(1.4);
        let l = ring.brightness * (0.20 + fade * 0.60);
        if l < 0.02 {
            continue;
        }
        let base_col = brighten(theme_palette(ring.hue), l.clamp(0.0, 0.85));

        let steps = ((2.0 * PI_F * ring.radius) as usize).max(24).min(360);
        for s in 0..steps {
            let a = s as f32 / steps as f32 * 2.0 * PI_F;
            let x = cx + a.cos() * ring.radius * aspect;
            let y = cy + a.sin() * ring.radius;
            plot(&mut cells, &mut cell_color, x as i32, y as i32, base_col);
        }
    }

    // Bright center pinhole for the "moving forward" feel.
    let center_col = brighten(theme_palette(0.95), 0.80);
    for oy in -1..=1 {
        for ox in -2..=2 {
            plot(&mut cells, &mut cell_color, cx as i32 + ox, cy as i32 + oy, center_col);
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                rgb(cell_color[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_radar(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Radar ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 6 || inner.height < 6 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;
    let aspect = 2.0_f32;
    let cx = dx as f32 / 2.0;
    let cy = dy as f32 / 2.0;
    let r_outer = (dy as f32 * 0.5).min(dx as f32 / aspect * 0.5) - 1.0;
    let r_inner = 1.5_f32;

    // Re-init persistent paint buffer when size changes.
    if app.radar_grid_w != dx || app.radar_grid_h != dy {
        app.radar_grid_w = dx;
        app.radar_grid_h = dy;
        app.radar_grid = vec![0.0; dx * dy];
        app.radar_blips.clear();
    }

    // Decay the existing paint.
    for v in app.radar_grid.iter_mut() {
        *v *= 0.90;
    }

    let bins = 64;
    let samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&samples, bins);

    // Track overall and bass energy for reactive sweep speed and core pulse.
    let total: f32 = spec.iter().sum::<f32>() / bins as f32;
    let bass: f32 = spec[..bins / 8].iter().sum::<f32>() / (bins / 8) as f32;
    app.radar_energy = app.radar_energy * 0.85 + total * 0.15;
    if bass > app.radar_bass {
        app.radar_bass = bass;
    } else {
        app.radar_bass *= 0.88;
    }
    app.radar_pulse = (app.radar_pulse + 0.18 + app.radar_bass * 0.4).rem_euclid(TAU_F);

    // Advance the spoke and paint the wedge swept since last frame.
    let prev_angle = app.radar_angle;
    let speed = 0.040 + app.radar_energy * 0.10;
    let new_angle = (prev_angle + speed).rem_euclid(TAU_F);
    let sweep_steps = 8;
    let radial_steps = (r_outer - r_inner).ceil() as usize * 2;
    for step in 0..=sweep_steps {
        let t = step as f32 / sweep_steps as f32;
        let a = prev_angle + speed * t;
        let ca = a.cos();
        let sa = a.sin();
        for rs in 0..=radial_steps {
            let frac = rs as f32 / radial_steps as f32;
            let r = r_inner + frac * (r_outer - r_inner);
            // Frequency increases outward.
            let bin = (frac * (bins - 1) as f32) as usize;
            let mag = spec[bin].clamp(0.0, 1.0);
            if mag < 0.04 {
                continue;
            }
            let x = cx + ca * r * aspect;
            let y = cy + sa * r;
            let xi = x as i32;
            let yi = y as i32;
            if xi < 0 || yi < 0 || xi >= dx as i32 || yi >= dy as i32 {
                continue;
            }
            let idx = yi as usize * dx + xi as usize;
            if mag > app.radar_grid[idx] {
                app.radar_grid[idx] = mag;
            }
            // Spawn a contact ping at the leading edge of the sweep when we
            // hit a strong return — gated to avoid flooding.
            if step == sweep_steps && mag > 0.55 && rs % 6 == 0 && app.radar_blips.len() < 32 {
                app.radar_blips.push(RadarBlip {
                    x,
                    y,
                    age: 0.0,
                    intensity: mag,
                    hue: 0.85 - frac * 0.55,
                });
            }
        }
    }
    app.radar_angle = new_angle;

    // Age and prune blips.
    for b in app.radar_blips.iter_mut() {
        b.age += 1.0;
    }
    app.radar_blips.retain(|b| b.age < 24.0);

    let mut cells = vec![0u8; cw * ch];
    let mut cell_color = vec![[0u8; 3]; cw * ch];
    // Marks cells claimed by signal layers (sweep trail, afterglow, blips,
    // core). Underlay grid only paints where this is false, so it never
    // competes with informational pixels.
    let mut signal = vec![false; cw * ch];

    // Paint a sub-pixel as part of the signal layer (overwrites by luma-max).
    let put_signal = |cells: &mut [u8],
                      cell_color: &mut [[u8; 3]],
                      signal: &mut [bool],
                      xi: i32,
                      yi: i32,
                      col: [u8; 3]| {
        if xi < 0 || yi < 0 || xi >= dx as i32 || yi >= dy as i32 {
            return;
        }
        let xu = xi as usize;
        let yu = yi as usize;
        let idx = (yu / 4) * cw + (xu / 2);
        cells[idx] |= dot_bit(xu % 2, yu % 4);
        signal[idx] = true;
        if luma(col) > luma(cell_color[idx]) {
            cell_color[idx] = col;
        }
    };

    // Render the persistent paint buffer (afterglow of the sweep).
    for sy in 0..dy {
        for sx in 0..dx {
            let v = app.radar_grid[sy * dx + sx];
            if v < 0.05 {
                continue;
            }
            let cell_x = sx / 2;
            let cell_y = sy / 4;
            let idx = cell_y * cw + cell_x;
            cells[idx] |= dot_bit(sx % 2, sy % 4);
            signal[idx] = true;
            let xr = (sx as f32 - cx) / aspect;
            let yr = sy as f32 - cy;
            let dist = (xr * xr + yr * yr).sqrt();
            let dist_norm = (dist / r_outer).clamp(0.0, 1.0);
            let hue = 0.85 - dist_norm * 0.55;
            let l = (0.25 + v * 0.70).clamp(0.0, 1.0);
            let col = brighten(theme_palette(hue), l);
            if luma(col) > luma(cell_color[idx]) {
                cell_color[idx] = col;
            }
        }
    }

    // Tight, sharp sweep beam — short trail with steep falloff so the head
    // really pops against the afterglow.
    let trail_count = 8;
    let trail_arc = 0.32_f32;
    let head_steps = ((r_outer - r_inner) * 1.6) as usize;
    for t in (0..trail_count).rev() {
        let frac = t as f32 / trail_count as f32;
        let a = app.radar_angle - frac * trail_arc;
        let ca = a.cos();
        let sa = a.sin();
        let bright = (1.0 - frac).powf(2.4);
        let l = if t == 0 { 1.0 } else { 0.15 + bright * 0.55 };
        let hue = if t == 0 { 1.00 } else { 0.92 };
        let col = brighten(theme_palette(hue), l);
        for s in 0..=head_steps {
            let r = r_inner + (s as f32 / head_steps as f32) * (r_outer - r_inner);
            let x = cx + ca * r * aspect;
            let y = cy + sa * r;
            put_signal(&mut cells, &mut cell_color, &mut signal, x as i32, y as i32, col);
        }
    }

    // Expanding contact-ping rings for blips — drawn last so they sit on top.
    for b in &app.radar_blips {
        let life = b.age / 24.0;
        let radius = 1.0 + life * 9.0 * (0.6 + b.intensity * 0.6);
        let bright = (1.0 - life).powf(1.2) * (0.65 + b.intensity * 0.35);
        if bright < 0.08 {
            continue;
        }
        let circ = (TAU_F * radius * aspect) as usize;
        let steps = circ.max(16);
        let col = brighten(theme_palette(b.hue), bright);
        for s in 0..steps {
            let a = (s as f32 / steps as f32) * TAU_F;
            let x = b.x + a.cos() * radius * aspect;
            let y = b.y + a.sin() * radius;
            put_signal(&mut cells, &mut cell_color, &mut signal, x as i32, y as i32, col);
        }
    }

    // Pulsing bass-reactive core.
    let core_r = 1.0 + app.radar_bass * 4.5 + (app.radar_pulse.sin() * 0.5 + 0.5) * 1.2;
    let core_col = brighten(theme_palette(0.98), (0.65 + app.radar_bass * 0.35).min(1.0));
    let core_steps = ((core_r * aspect * TAU_F) as usize).max(12);
    for s in 0..core_steps {
        let a = (s as f32 / core_steps as f32) * TAU_F;
        for rr in [core_r * 0.4, core_r * 0.75, core_r] {
            let x = cx + a.cos() * rr * aspect;
            let y = cy + a.sin() * rr;
            put_signal(&mut cells, &mut cell_color, &mut signal, x as i32, y as i32, core_col);
        }
    }

    // CRT scope underlay: very dim range rings + crosshair axes, painted
    // ONLY where there's no signal so they recede into the background.
    let put_under = |cells: &mut [u8],
                     cell_color: &mut [[u8; 3]],
                     signal: &[bool],
                     xi: i32,
                     yi: i32,
                     col: [u8; 3]| {
        if xi < 0 || yi < 0 || xi >= dx as i32 || yi >= dy as i32 {
            return;
        }
        let xu = xi as usize;
        let yu = yi as usize;
        let idx = (yu / 4) * cw + (xu / 2);
        if signal[idx] {
            return;
        }
        cells[idx] |= dot_bit(xu % 2, yu % 4);
        cell_color[idx] = col;
    };
    let ring_col = brighten(theme_palette(0.05), 0.10);
    for k in 1..=3 {
        let rr = r_outer * (k as f32 / 3.0);
        let circ = (TAU_F * rr * aspect) as usize;
        let steps = circ.max(64);
        for s in 0..steps {
            let a = (s as f32 / steps as f32) * TAU_F;
            let x = cx + a.cos() * rr * aspect;
            let y = cy + a.sin() * rr;
            put_under(&mut cells, &mut cell_color, &signal, x as i32, y as i32, ring_col);
        }
    }
    let axis_col = brighten(theme_palette(0.05), 0.12);
    let axis_steps = (r_outer * 1.2) as usize;
    for s in 0..=axis_steps {
        let r = r_inner + (s as f32 / axis_steps as f32) * (r_outer - r_inner);
        for (ux, uy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let x = cx + ux * r * aspect;
            let y = cy + uy * r;
            put_under(&mut cells, &mut cell_color, &signal, x as i32, y as i32, axis_col);
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                rgb(cell_color[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

const TAU_F: f32 = std::f32::consts::TAU;

fn draw_mandala(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Mandala ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 6 || inner.height < 6 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;
    let aspect = 2.0_f32;
    let cx = dx as f32 / 2.0;
    let cy = dy as f32 / 2.0;
    let r_max = (dy as f32 * 0.5).min(dx as f32 / aspect * 0.5) - 1.0;
    let r_max = r_max.max(2.0);

    // Spectrum drives radial bands. Smoothed for stability.
    let bins = 24;
    let samples = app.tap.snapshot(2048);
    let mags = app.spectrum.analyze(&samples, bins);
    if app.mandala_smoothed.len() != bins {
        app.mandala_smoothed = vec![0.0; bins];
    }
    for (i, m) in mags.iter().enumerate() {
        let prev = app.mandala_smoothed[i];
        app.mandala_smoothed[i] = if *m > prev {
            prev + (*m - prev) * 0.45
        } else {
            prev * 0.88
        };
    }

    // Phase scales with overall energy so the mandala spins faster on loud.
    let energy: f32 = app.mandala_smoothed.iter().sum::<f32>() / bins as f32;
    app.mandala_phase = (app.mandala_phase + 0.012 + energy * 0.06).rem_euclid(TAU_F);

    let mut cells = vec![0u8; cw * ch];
    let mut cell_color = vec![[0u8; 3]; cw * ch];

    // 8-fold rotational symmetry: cos(8θ + …) gives 8 lobes; r-coupled phase
    // adds the swirling braid.
    for sy in 0..dy {
        for sx in 0..dx {
            let xr = (sx as f32 - cx) / aspect;
            let yr = sy as f32 - cy;
            let r = (xr * xr + yr * yr).sqrt();
            if r > r_max || r < 0.5 {
                continue;
            }
            let a = yr.atan2(xr);
            let rn = (r / r_max).clamp(0.0, 1.0);
            let bin = (rn * (bins - 1) as f32) as usize;
            let mag = app.mandala_smoothed[bin].clamp(0.0, 1.0);
            if mag < 0.05 {
                continue;
            }
            let petal = (8.0 * a + app.mandala_phase + rn * 7.0).cos();
            let petal2 = (16.0 * a - app.mandala_phase * 0.7 + rn * 4.0).cos();
            let pattern = (petal * 0.6 + petal2 * 0.4) * 0.5 + 0.5;
            let intensity = mag * pattern.powf(2.2);
            if intensity < 0.18 {
                continue;
            }

            let cell_x = sx / 2;
            let cell_y = sy / 4;
            let idx = cell_y * cw + cell_x;
            cells[idx] |= dot_bit(sx % 2, sy % 4);

            let hue = (rn + app.mandala_phase / TAU_F).rem_euclid(1.0);
            let l = (0.25 + intensity * 0.65).clamp(0.0, 0.92);
            let col = brighten(theme_palette(hue), l);
            if luma(col) > luma(cell_color[idx]) {
                cell_color[idx] = col;
            }
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                rgb(cell_color[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_quake(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Quake ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 6 || inner.height < 6 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;
    let aspect = 2.0_f32;
    let cx = dx as f32 / 2.0;
    let cy = dy as f32 / 2.0;
    let r_max = (dy as f32 * 0.55).min(dx as f32 / aspect * 0.55).max(4.0);

    // Bass band drives shockwaves.
    let bins = 32;
    let samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&samples, bins);
    let bass: f32 = spec[..bins / 8].iter().copied().sum::<f32>()
        / (bins / 8).max(1) as f32;

    // Running average for adaptive threshold.
    let prev_avg = app.quake_bass_avg;
    app.quake_bass_avg = prev_avg * 0.92 + bass * 0.08;
    let trigger = bass > 0.28 && bass > prev_avg * 1.55;
    if trigger {
        let hue = 0.78 + rand_f32(&mut app.quake_rng) * 0.18;
        app.quake_rings.push(QuakeRing {
            radius: 0.5,
            intensity: 1.0,
            hue,
        });
        app.quake_shake = (app.quake_shake + bass).min(2.5);
    }

    // Advance rings.
    let ring_speed = 0.9 + bass * 1.0;
    for ring in &mut app.quake_rings {
        ring.radius += ring_speed;
        ring.intensity *= 0.94;
    }
    app.quake_rings
        .retain(|r| r.intensity > 0.05 && r.radius < r_max + 4.0);
    if app.quake_rings.len() > 16 {
        let drop = app.quake_rings.len() - 16;
        app.quake_rings.drain(0..drop);
    }

    let mut cells = vec![0u8; cw * ch];
    let mut cell_color = vec![[0u8; 3]; cw * ch];

    // Random per-frame jitter offset, magnitude decays with quake_shake.
    let shake_mag = app.quake_shake;
    let shake_x = (rand_f32(&mut app.quake_rng) - 0.5) * 2.0 * shake_mag * aspect;
    let shake_y = (rand_f32(&mut app.quake_rng) - 0.5) * 2.0 * shake_mag;
    app.quake_shake *= 0.78;

    // Draw each ring: thick band of braille dots around its radius.
    for ring in &app.quake_rings {
        let r = ring.radius;
        // Thickness shrinks as ring expands so the leading edge stays defined.
        let thickness = (2.5 + ring.intensity * 2.5).max(1.0);
        let steps = ((TAU_F * r) as usize).max(48);
        for s in 0..steps {
            let a = s as f32 / steps as f32 * TAU_F;
            let ca = a.cos();
            let sa = a.sin();
            // Plot a few radial offsets for the band thickness.
            let layers = thickness.ceil() as i32;
            for t in -layers..=layers {
                let t_off = t as f32 * 0.5;
                let rr = r + t_off;
                if rr < 1.0 || rr > r_max + 4.0 {
                    continue;
                }
                let band = 1.0 - (t_off.abs() / thickness).min(1.0);
                let x = cx + ca * rr * aspect + shake_x;
                let y = cy + sa * rr + shake_y;
                if x < 0.0 || y < 0.0 {
                    continue;
                }
                let xi = x as usize;
                let yi = y as usize;
                if xi >= dx || yi >= dy {
                    continue;
                }
                let cell_x = xi / 2;
                let cell_y = yi / 4;
                let idx = cell_y * cw + cell_x;
                cells[idx] |= dot_bit(xi % 2, yi % 4);
                let l = (0.20 + ring.intensity * band * 0.70).clamp(0.0, 0.95);
                let col = brighten(theme_palette(ring.hue), l);
                if luma(col) > luma(cell_color[idx]) {
                    cell_color[idx] = col;
                }
            }
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                DIM_STEEL()
            } else {
                rgb(cell_color[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_lightning(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Lightning ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw * 2;
    let dy = ch * 4;

    // Use raw amplitude transients to fire bolts. Peak-hold gate (not EMA):
    // the hold decays each frame, so the next loud sample retriggers easily,
    // whereas an EMA gets dragged up by the strike itself and locks us out.
    let amp_samples = app.tap.snapshot(512);
    let mut inst_peak = 0.0f32;
    for s in &amp_samples {
        let a = s.abs();
        if a > inst_peak {
            inst_peak = a;
        }
    }
    let prev_hold = app.lightning_avg;
    let trigger = inst_peak > 0.18 && inst_peak > prev_hold + 0.08;
    if trigger {
        app.lightning_avg = inst_peak;
    } else {
        app.lightning_avg = (prev_hold * 0.90).max(inst_peak);
    }

    if trigger {
        // Number of bolts scales with intensity.
        let n_bolts = 1 + (inst_peak * 3.0) as usize;
        for _ in 0..n_bolts.min(4) {
            let bolt = generate_bolt(dx, dy, inst_peak, &mut app.lightning_rng);
            app.lightning_bolts.push(bolt);
        }
        app.lightning_life = 1.0;
    } else {
        app.lightning_life *= 0.72;
    }
    // Drop bolts whose collective life is gone.
    if app.lightning_life < 0.05 {
        app.lightning_bolts.clear();
    }
    if app.lightning_bolts.len() > 8 {
        let drop = app.lightning_bolts.len() - 8;
        app.lightning_bolts.drain(0..drop);
    }

    let mut cells = vec![0u8; cw * ch];
    let mut cell_color = vec![[0u8; 3]; cw * ch];

    // Faint flash overlay: ambient brightening on the whole canvas while a
    // bolt is alive.
    let flash = app.lightning_life;

    let plot = |cells: &mut [u8], cell_color: &mut [[u8; 3]], x: i32, y: i32, l: f32, hue: f32| {
        if x < 0 || y < 0 || x >= dx as i32 || y >= dy as i32 {
            return;
        }
        let xu = x as usize;
        let yu = y as usize;
        let cell_x = xu / 2;
        let cell_y = yu / 4;
        let idx = cell_y * cw + cell_x;
        cells[idx] |= dot_bit(xu % 2, yu % 4);
        let col = brighten(theme_palette(hue), l.clamp(0.0, 0.95));
        if luma(col) > luma(cell_color[idx]) {
            cell_color[idx] = col;
        }
    };

    for bolt in &app.lightning_bolts {
        for p in bolt {
            let l = (0.45 + p.brightness * 0.50) * (0.6 + flash * 0.4);
            plot(&mut cells, &mut cell_color, p.x, p.y, l, 0.92);
            // Glow: brighten the immediate left/right neighbors for thickness.
            plot(
                &mut cells,
                &mut cell_color,
                p.x + 1,
                p.y,
                l * 0.55,
                0.85,
            );
            plot(
                &mut cells,
                &mut cell_color,
                p.x - 1,
                p.y,
                l * 0.55,
                0.85,
            );
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans = Vec::with_capacity(cw);
        for col in 0..cw {
            let idx = row * cw + col;
            let byte = cells[idx];
            let glyph = if byte == 0 {
                ' '
            } else {
                char::from_u32(0x2800 + byte as u32).unwrap_or(' ')
            };
            let fg = if byte == 0 {
                // Subtle flash background tint when a bolt just fired.
                if flash > 0.15 {
                    rgb(brighten(theme_palette(0.20), 0.18 * flash))
                } else {
                    DIM_STEEL()
                }
            } else {
                rgb(cell_color[idx])
            };
            spans.push(Span::styled(glyph.to_string(), Style::default().fg(fg)));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn generate_bolt(dx: usize, dy: usize, intensity: f32, rng: &mut u64) -> Vec<LightningPoint> {
    let mut points = Vec::with_capacity(dy + 16);
    let start_x = (rand_f32(rng) * dx as f32) as i32;
    let mut x = start_x as f32;
    // Bias the horizontal drift so the whole bolt veers in one general direction.
    let bias = (rand_f32(rng) - 0.5) * 1.4;
    for y in 0..dy as i32 {
        let jitter = (rand_f32(rng) - 0.5) * 3.4;
        x += jitter + bias * 0.15;
        let xi = x as i32;
        points.push(LightningPoint {
            x: xi,
            y,
            brightness: 1.0,
        });
        // Random short forks at ~6% of steps.
        if rand_f32(rng) < 0.06 + intensity * 0.05 {
            let fork_len = 4 + (rand_f32(rng) * 14.0) as i32;
            let fork_dir = if rand_f32(rng) > 0.5 { 1.0 } else { -1.0 };
            let mut fx = x;
            for fy in 1..=fork_len {
                fx += fork_dir * (1.0 + rand_f32(rng) * 1.2);
                let fyi = y + fy;
                if fyi >= dy as i32 {
                    break;
                }
                points.push(LightningPoint {
                    x: fx as i32,
                    y: fyi,
                    brightness: 0.55 - (fy as f32 / fork_len as f32) * 0.35,
                });
            }
        }
    }
    points
}

fn draw_julia(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Julia ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 6 || inner.height < 4 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    // Half-block: each terminal cell is two vertical sub-pixels.
    let dx = cw;
    let dy = ch * 2;

    // Drive c from the spectrum: bass → orbit radius, treble → orbit speed,
    // mids → imaginary-axis skew. Slow ellipse around the seahorse anchor.
    let bins = 24;
    let samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&samples, bins);
    let bass: f32 = spec[..bins / 6].iter().copied().sum::<f32>()
        / (bins / 6).max(1) as f32;
    let mids: f32 = spec[bins / 6..bins / 2].iter().copied().sum::<f32>()
        / ((bins / 2) - (bins / 6)).max(1) as f32;
    let treble: f32 = spec[bins / 2..].iter().copied().sum::<f32>()
        / (bins - bins / 2).max(1) as f32;

    let speed = 0.006 + treble * 0.045;
    app.julia_phase = (app.julia_phase + speed).rem_euclid(TAU_F);

    // Anchor in the seahorse-valley region; bass swings c on an ellipse, mids
    // nudge the imaginary axis so the shape breathes asymmetrically.
    let radius = 0.26 + bass * 0.18;
    let cx_target = -0.4 + radius * app.julia_phase.cos();
    let cy_target = 0.6 + radius * 0.85 * (app.julia_phase * 0.83).sin()
        + (mids - 0.3) * 0.10;
    // Smooth so c never jumps; sudden c changes look like discrete frame jitter.
    app.julia_cx += (cx_target - app.julia_cx) * 0.22;
    app.julia_cy += (cy_target - app.julia_cy) * 0.22;
    let cx_param = app.julia_cx;
    let cy_param = app.julia_cy;

    // Map sub-pixels to complex plane: square sample region, scaled to the
    // smaller dimension so the fractal isn't stretched.
    let min_dim = dx.min(dy) as f32;
    let view = 3.2_f32; // visible width/height in complex units
    let half = view * 0.5;
    let cx_screen = dx as f32 * 0.5;
    let cy_screen = dy as f32 * 0.5;

    let max_iter: u32 = 48;
    let ln2 = std::f32::consts::LN_2;

    // sub-pixel intensity buffer: row-major dx × dy of (escape_t, in_set)
    // where escape_t is the smoothed iteration count normalized to [0,1].
    let mut grid = vec![(0.0f32, false); dx * dy];

    for sy in 0..dy {
        for sx in 0..dx {
            let zx0 = (sx as f32 - cx_screen) / (min_dim * 0.5) * half;
            let zy0 = (sy as f32 - cy_screen) / (min_dim * 0.5) * half;
            let mut zx = zx0;
            let mut zy = zy0;
            let mut iter: u32 = 0;
            let mut mag2 = 0.0f32;
            while iter < max_iter {
                let zx2 = zx * zx;
                let zy2 = zy * zy;
                mag2 = zx2 + zy2;
                if mag2 > 16.0 {
                    break;
                }
                let new_zx = zx2 - zy2 + cx_param;
                let new_zy = 2.0 * zx * zy + cy_param;
                zx = new_zx;
                zy = new_zy;
                iter += 1;
            }
            let in_set = iter >= max_iter;
            let smooth = if in_set {
                0.0
            } else {
                // Continuous coloring: removes banding at the boundary.
                let log_zn = mag2.ln() * 0.5;
                let nu = (log_zn / ln2).ln() / ln2;
                let s = iter as f32 + 1.0 - nu;
                (s / max_iter as f32).clamp(0.0, 1.0)
            };
            grid[sy * dx + sx] = (smooth, in_set);
        }
    }

    // Palette rotation drifts with the orbit phase — gives the colors a slow
    // global hue shift independent of the fractal motion.
    let hue_shift = app.julia_phase / TAU_F * 0.5;

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let top_sub = row * 2;
        let bot_sub = row * 2 + 1;
        let mut spans: Vec<Span> = Vec::with_capacity(cw);
        for col in 0..cw {
            let (t_top, in_top) = grid[top_sub * dx + col];
            let (t_bot, in_bot) = grid[bot_sub * dx + col];

            // Both pixels inside the set: render a dark cell so the in-set
            // silhouette stays solid and instantly readable.
            if in_top && in_bot {
                spans.push(Span::styled(
                    " ".to_string(),
                    Style::default().fg(DIM_STEEL()),
                ));
                continue;
            }

            let color_for = |t: f32, in_set: bool| -> [u8; 3] {
                if in_set {
                    return [4, 4, 8];
                }
                // Pull t off the dead-flat low end so dim escape rings still
                // sit on a visible palette stop.
                let hue = (t.powf(0.7) + hue_shift).rem_euclid(1.0);
                let l = (0.22 + (1.0 - t) * 0.55).clamp(0.0, 0.92);
                brighten(theme_palette(hue), l)
            };

            let fg = color_for(t_top, in_top);
            let bg = color_for(t_bot, in_bot);
            spans.push(Span::styled(
                "▀".to_string(),
                Style::default().fg(rgb(fg)).bg(rgb(bg)),
            ));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Heart implicit curve: f(x,y) = (x²+y²-1)³ - x²y³. Interior is f<0.
/// In math coords the point sits at (0,-1) and the lobes flare around y≈1, so
/// callers should flip screen-y to render the heart upright.
fn heart_signed(x: f32, y: f32) -> f32 {
    let s = x * x + y * y - 1.0;
    s * s * s - x * x * y * y * y
}

fn draw_heart(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Heart ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 6 || inner.height < 4 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let dx = cw;
    let dy = ch * 2; // half-block: each cell stacks two sub-rows

    // --- Audio analysis ---
    let bins = 24;
    let samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&samples, bins);
    let bass: f32 =
        spec[..bins / 6].iter().copied().sum::<f32>() / (bins / 6).max(1) as f32;
    let total: f32 = spec.iter().copied().sum::<f32>() / bins as f32;

    // Peak-hold bass gate. Each accepted kick spawns a heart-shaped ripple and
    // boosts the next-frame pulse. The hold decays so closely-spaced beats fire.
    let prev_hold = app.heart_bass_hold;
    let kicked = bass > 0.20 && bass > prev_hold + 0.08;
    if kicked {
        app.heart_bass_hold = bass;
        app.heart_kick = (app.heart_kick + bass).min(1.5);
        app.heart_ripples.push(HeartRipple {
            c: 0.0,
            intensity: 1.0,
        });
    } else {
        app.heart_bass_hold = (prev_hold * 0.92).max(bass);
    }

    // Idle ~60 bpm pulse so the heart still beats with no audio. Peaky shape
    // (sin clipped to positive, then ^4) so the rest phase is long and the
    // contraction is sharp — reads as a real heartbeat.
    app.heart_idle_phase = (app.heart_idle_phase + 0.10).rem_euclid(TAU_F);
    let idle = app.heart_idle_phase.sin().max(0.0).powf(4.0) * 0.13;

    let kick_amp = app.heart_kick * 0.45;
    let target_scale = 1.0 + idle + kick_amp + bass * 0.30;
    app.heart_scale += (target_scale - app.heart_scale) * 0.32;
    app.heart_kick *= 0.78;

    // Hue drift: slow base rotation, faster on energy.
    app.heart_hue_phase =
        (app.heart_hue_phase + 0.004 + total * 0.020).rem_euclid(1.0);

    // Advance and cull ripples.
    for ripple in &mut app.heart_ripples {
        ripple.c += 0.075 + bass * 0.06;
        ripple.intensity *= 0.945;
    }
    app.heart_ripples
        .retain(|r| r.intensity > 0.04 && r.c < 8.0);
    if app.heart_ripples.len() > 10 {
        let drop = app.heart_ripples.len() - 10;
        app.heart_ripples.drain(0..drop);
    }

    // --- Sample the heart-coordinate plane ---
    // The heart's natural extent: x∈[-1.4,1.4], y∈[-1.0,1.3]. Pick a unit that
    // fits comfortably at idle scale and grows/shrinks with the pulse.
    let scale_basis = (dx as f32).min(dy as f32) * 0.5;
    let unit = scale_basis * 0.55 * app.heart_scale;
    let cx = dx as f32 * 0.5;
    let cy = dy as f32 * 0.52; // bias slightly down: heart "sits" better off-center

    let mut intensities = vec![0.0f32; dx * dy];
    let mut hues = vec![0.0f32; dx * dy];

    let base_hue = (app.heart_hue_phase + 0.88).rem_euclid(1.0);

    for sy in 0..dy {
        for sx in 0..dx {
            let nx = (sx as f32 - cx) / unit;
            let ny = -((sy as f32 - cy) / unit); // flip to render upright
            let f = heart_signed(nx, ny);
            let idx = sy * dx + sx;

            if f < 0.0 {
                // Interior: depth-shaded fill that intensifies the deeper you go.
                let depth = (-f).clamp(0.0, 1.0);
                intensities[idx] = (0.55 + depth * 0.40).min(1.0);
                // Tiny hue shift across depth gives the heart subtle dimension.
                hues[idx] = base_hue + depth * 0.04;
            } else {
                // Outside: paint ripples (iso-contours of f).
                let mut best = 0.0f32;
                let mut best_age = 0.0f32;
                for ripple in &app.heart_ripples {
                    // Tolerance widens as the ripple ages so it stays visible
                    // when the heart-curve gradient steepens with distance.
                    let tol = 0.10 + ripple.c * 0.07;
                    let dist = (f - ripple.c).abs();
                    if dist < tol {
                        let i = (1.0 - dist / tol) * ripple.intensity;
                        if i > best {
                            best = i;
                            best_age = ripple.c;
                        }
                    }
                }
                if best > 0.04 {
                    intensities[idx] = best * 0.88;
                    // Ripples cool as they expand: shift hue toward the dim end.
                    hues[idx] = (base_hue - 0.05 - best_age * 0.02).rem_euclid(1.0);
                }
            }
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let top_sub = row * 2;
        let bot_sub = row * 2 + 1;
        let mut spans: Vec<Span> = Vec::with_capacity(cw);
        for col in 0..cw {
            let it = intensities[top_sub * dx + col];
            let ib = intensities[bot_sub * dx + col];
            if it < 0.04 && ib < 0.04 {
                spans.push(Span::styled(
                    " ".to_string(),
                    Style::default().fg(DIM_STEEL()),
                ));
                continue;
            }
            let ht = hues[top_sub * dx + col].rem_euclid(1.0);
            let hb = hues[bot_sub * dx + col].rem_euclid(1.0);
            let fg = brighten(theme_palette(ht), (0.22 + it * 0.70).clamp(0.0, 0.95));
            let bg = brighten(theme_palette(hb), (0.22 + ib * 0.70).clamp(0.0, 0.95));
            spans.push(Span::styled(
                "▀".to_string(),
                Style::default().fg(rgb(fg)).bg(rgb(bg)),
            ));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}


fn draw_eye(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(STEEL()))
        .title(Span::styled(
            " Eye ",
            Style::default().fg(ELECTRIC()).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 8 || inner.height < 4 {
        return;
    }

    let cw = inner.width as usize;
    let ch = inner.height as usize;
    let cx = cw as f32 * 0.5;
    let cy = ch as f32 * 0.5;

    // --- Audio analysis ---
    let bins = 32;
    let samples = app.tap.snapshot(2048);
    let spec = app.spectrum.analyze(&samples, bins);
    let bass: f32 =
        spec[..bins / 6].iter().copied().sum::<f32>() / (bins / 6).max(1) as f32;
    let mids: f32 = spec[bins / 6..bins / 2].iter().copied().sum::<f32>()
        / ((bins / 2) - (bins / 6)).max(1) as f32;
    let treble: f32 = spec[bins / 2..].iter().copied().sum::<f32>()
        / (bins - bins / 2).max(1) as f32;
    let total: f32 = spec.iter().copied().sum::<f32>() / bins as f32;

    // Soft idle breath — gentler than the old sin³ pump.
    app.eye_idle_phase = (app.eye_idle_phase + 0.035).rem_euclid(TAU_F);
    let idle = app.eye_idle_phase.sin().max(0.0).powf(2.0) * 0.10;

    let target_pupil = 0.22 + bass * 0.22 + idle * 0.3;
    app.eye_pupil += (target_pupil - app.eye_pupil) * 0.18;

    app.eye_iris_phase =
        (app.eye_iris_phase + 0.010 + total * 0.025 + treble * 0.015).rem_euclid(TAU_F);
    app.eye_hue_phase = (app.eye_hue_phase + 0.003 + mids * 0.010).rem_euclid(1.0);
    app.eye_glow = app.eye_glow * 0.90 + total * 0.10;

    let p_iris = app.eye_iris_phase;
    let base_hue = app.eye_hue_phase;

    // Iris fills the available area as an ellipse (char cells are ~2:1 h:w
    // so we work in unit-normalized coords). No eyelid, no sclera ring.
    let rx = cw as f32 * 0.50;
    let ry = ch as f32 * 0.50;
    let r_iris_norm = 0.95 + idle * 0.05;
    let r_pupil_norm = r_iris_norm * app.eye_pupil;

    // Density ramp — sparser glyphs at the periphery give the dotted/transparent feel.
    let dots: [&str; 6] = [" ", "·", "·", "∙", "•", "●"];

    let mut lines: Vec<Line> = Vec::with_capacity(ch);
    for row in 0..ch {
        let mut spans: Vec<Span> = Vec::with_capacity(cw);
        for col in 0..cw {
            let dxf = (col as f32 + 0.5 - cx) / rx;
            let dyf = (row as f32 + 0.5 - cy) / ry;
            let r = (dxf * dxf + dyf * dyf).sqrt();

            if r > r_iris_norm {
                spans.push(Span::styled(" ", Style::default().fg(DIM_STEEL())));
                continue;
            }

            let r_norm = (r / r_iris_norm).clamp(0.0, 1.0);
            let angle = dyf.atan2(dxf);

            // Gentle radial swirl — much softer than the old striations.
            let swirl = (r_norm * 1.8 + p_iris * 0.6).sin() * 0.32;
            let twisted = angle + swirl;

            let bin_pos = (twisted / TAU_F + 0.5).rem_euclid(1.0);
            let bin = ((bin_pos * bins as f32) as usize).min(bins - 1);
            let mag = spec[bin].clamp(0.0, 1.0);

            let h1 = bin_pos;
            let h2 = (r_norm * 0.45 + base_hue * 1.3 + p_iris * 0.06).rem_euclid(1.0);
            let hue = (h1 * 0.55 + h2 * 0.45 + base_hue).rem_euclid(1.0);

            // Pupil: smooth fade so the centre stays inky without a hard hole.
            let pupil_fade =
                ((r - r_pupil_norm * 0.7) / (r_pupil_norm * 0.6 + 0.02)).clamp(0.0, 1.0);
            // Edge fade so dots thin out at the iris rim.
            let edge_fade = (1.0 - r_norm.powf(1.6)).clamp(0.0, 1.0);

            // Static per-cell jitter breaks the grid without flickering.
            let stipple = stipple_hash(col, row);

            let intensity = (0.32
                + mag * 0.30
                + app.eye_glow * 0.10
                + ((r_norm * 5.0 - p_iris * 0.7).cos() * 0.5 + 0.5) * 0.10
                + (stipple - 0.5) * 0.30)
                * edge_fade
                * pupil_fade.powf(1.1);
            let intensity = intensity.clamp(0.0, 1.0);

            let tier = (intensity * (dots.len() as f32 - 0.001)) as usize;
            let glyph = dots[tier.min(dots.len() - 1)];
            if glyph == " " {
                spans.push(Span::styled(" ", Style::default().fg(DIM_STEEL())));
                continue;
            }

            let l = (0.30 + mag * 0.14 + app.eye_glow * 0.05).clamp(0.24, 0.55);
            let color = brighten(theme_palette(hue), l);
            spans.push(Span::styled(
                glyph.to_string(),
                Style::default().fg(rgb(color)),
            ));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(
        Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center),
        inner,
    );
}

fn stipple_hash(x: usize, y: usize) -> f32 {
    let mut n = (x as u32).wrapping_mul(0x27d4eb2d)
        ^ (y as u32).wrapping_mul(0x165667b1);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85ebca6b);
    n ^= n >> 13;
    (n as f32 / u32::MAX as f32).clamp(0.0, 1.0)
}
