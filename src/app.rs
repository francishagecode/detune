use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use crate::library::{self, Entry};
use crate::metadata::{self, TrackMeta};
use crate::mouse::{ClickTarget, HitMap};
use crate::player::{spawn_seek_worker, Player, SeekRequest, SeekResult};
use crate::theme::{self, THEMES};
use crate::visualizer::{
    AudioTap, DriftPoint, HeartRipple, LightningPoint, Particle, QuakeRing, RadarBlip, Spectrum,
    TunnelRing, VizMode,
};

pub struct App {
    pub should_quit: bool,
    pub player: Player,
    pub root: PathBuf,
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub scroll: usize,
    pub now_playing: Option<PathBuf>,
    pub hovered: Option<usize>,
    pub track_duration: Option<Duration>,
    pub elapsed: Duration,
    last_tick: Instant,
    pub hit_map: HitMap,
    pub status: String,
    pub picker: Option<Picker>,
    pub meta_cache: HashMap<PathBuf, TrackMeta>,
    pub image_cache: HashMap<PathBuf, StatefulProtocol>,
    pub viz_mode: VizMode,
    pub tap: AudioTap,
    pub spectrum: Spectrum,
    pub viz_bar_peaks: Vec<f32>,
    pub chroma_smoothed: [f32; 12],
    pub spark_particles: Vec<Particle>,
    pub spark_peak_hold: f32,
    pub spark_rng: u64,
    pub drift_trail: VecDeque<DriftPoint>,
    pub drift_phase: f32,
    pub drift_x: f32,
    pub drift_y: f32,
    pub drift_vx: f32,
    pub drift_vy: f32,
    pub aurora_bands: [f32; 4],
    pub aurora_phase: f32,
    pub tunnel_rings: Vec<TunnelRing>,
    pub tunnel_spawn_timer: f32,
    pub tunnel_peak_hold: f32,
    pub radar_grid: Vec<f32>,
    pub radar_grid_w: usize,
    pub radar_grid_h: usize,
    pub radar_angle: f32,
    pub radar_blips: Vec<RadarBlip>,
    pub radar_energy: f32,
    pub radar_bass: f32,
    pub radar_pulse: f32,
    pub mandala_phase: f32,
    pub mandala_smoothed: Vec<f32>,
    pub quake_rings: Vec<QuakeRing>,
    pub quake_bass_avg: f32,
    pub quake_shake: f32,
    pub quake_rng: u64,
    pub lightning_bolts: Vec<Vec<LightningPoint>>,
    pub lightning_life: f32,
    pub lightning_avg: f32,
    pub lightning_rng: u64,
    pub julia_phase: f32,
    pub julia_cx: f32,
    pub julia_cy: f32,
    pub heart_scale: f32,
    pub heart_idle_phase: f32,
    pub heart_kick: f32,
    pub heart_bass_hold: f32,
    pub heart_hue_phase: f32,
    pub heart_ripples: Vec<HeartRipple>,
    pub eye_pupil: f32,
    pub eye_iris_phase: f32,
    pub eye_glow: f32,
    pub eye_hue_phase: f32,
    pub eye_idle_phase: f32,
    cwd_arc: Arc<Mutex<PathBuf>>,
    scan_rx: mpsc::Receiver<(PathBuf, Vec<Entry>)>,
    meta_req_tx: mpsc::Sender<PathBuf>,
    meta_res_rx: mpsc::Receiver<(PathBuf, Option<TrackMeta>)>,
    meta_pending: HashSet<PathBuf>,
    art_req_tx: mpsc::Sender<ArtRequest>,
    art_res_rx: mpsc::Receiver<(PathBuf, Option<DynamicImage>)>,
    art_pending: HashSet<PathBuf>,
    art_missing: HashSet<PathBuf>,
    last_preview: Option<PreviewTarget>,
    preview_pending: Option<(PreviewTarget, Instant)>,
    pub queue: VecDeque<PathBuf>,
    pub shuffle: bool,
    pub loop_one: bool,
    rng_state: u64,
    seek_req_tx: mpsc::Sender<SeekRequest>,
    seek_res_rx: mpsc::Receiver<SeekResult>,
    seek_id_counter: u64,
    pending_seek: Option<(u64, Duration)>,
    pub theme_idx: usize,
    pub theme_open: bool,
    pub theme_prev_idx: usize,
    pub theme_scroll: usize,
    pub viz_open: bool,
    pub viz_prev_mode: VizMode,
    pub viz_scroll: usize,
}

const SCAN_INTERVAL: Duration = Duration::from_secs(3);
const HOVER_DEBOUNCE: Duration = Duration::from_millis(120);

/// What the preview pane should show. The art cache is keyed by `art_key`
/// (the folder) so every track in a folder shares one decoded image; `art_source`
/// points at the actual file we read art from. `track` is set when previewing
/// a track (we have metadata to render); for a folder hover it's None and we
/// fall back to `fallback_label`.
#[derive(Clone, PartialEq, Eq)]
pub struct PreviewTarget {
    pub art_key: PathBuf,
    pub art_source: PathBuf,
    pub track: Option<PathBuf>,
    pub fallback_label: String,
}

pub struct ArtRequest {
    pub key: PathBuf,
    pub source: PathBuf,
}

impl App {
    pub fn new(root: PathBuf) -> Result<Self> {
        let root = root.canonicalize().unwrap_or(root);
        let cwd = root.clone();
        let entries = build_entries(&cwd, &root);
        let picker = Picker::from_query_stdio().ok();
        let player = Player::new()?;
        let tap = player.tap();
        let cwd_arc = Arc::new(Mutex::new(cwd.clone()));
        let scan_rx = spawn_scanner(cwd_arc.clone(), entries.clone());
        let (meta_req_tx, meta_res_rx) = spawn_meta_worker();
        let (art_req_tx, art_res_rx) = spawn_art_workers(2);
        let (seek_req_tx, seek_res_rx) = spawn_seek_worker(player.handle(), tap.clone());
        Ok(Self {
            should_quit: false,
            player,
            root,
            cwd,
            entries,
            selected: 0,
            scroll: 0,
            now_playing: None,
            hovered: None,
            track_duration: None,
            elapsed: Duration::ZERO,
            last_tick: Instant::now(),
            hit_map: HitMap::default(),
            status: String::new(),
            picker,
            meta_cache: HashMap::new(),
            image_cache: HashMap::new(),
            viz_mode: crate::visualizer::load_viz_mode(),
            tap,
            spectrum: Spectrum::new(1024),
            viz_bar_peaks: Vec::new(),
            chroma_smoothed: [0.0; 12],
            spark_particles: Vec::new(),
            spark_peak_hold: 0.0,
            spark_rng: seed_rng(),
            drift_trail: VecDeque::new(),
            drift_phase: 0.0,
            drift_x: 0.0,
            drift_y: 0.0,
            drift_vx: 0.0,
            drift_vy: 0.0,
            aurora_bands: [0.0; 4],
            aurora_phase: 0.0,
            tunnel_rings: Vec::new(),
            tunnel_spawn_timer: 0.0,
            tunnel_peak_hold: 0.0,
            radar_grid: Vec::new(),
            radar_grid_w: 0,
            radar_grid_h: 0,
            radar_angle: 0.0,
            radar_blips: Vec::new(),
            radar_energy: 0.0,
            radar_bass: 0.0,
            radar_pulse: 0.0,
            mandala_phase: 0.0,
            mandala_smoothed: Vec::new(),
            quake_rings: Vec::new(),
            quake_bass_avg: 0.0,
            quake_shake: 0.0,
            quake_rng: seed_rng(),
            lightning_bolts: Vec::new(),
            lightning_life: 0.0,
            lightning_avg: 0.0,
            lightning_rng: seed_rng(),
            julia_phase: 0.0,
            julia_cx: -0.4,
            julia_cy: 0.6,
            heart_scale: 1.0,
            heart_idle_phase: 0.0,
            heart_kick: 0.0,
            heart_bass_hold: 0.0,
            heart_hue_phase: 0.0,
            heart_ripples: Vec::new(),
            eye_pupil: 0.30,
            eye_iris_phase: 0.0,
            eye_glow: 0.0,
            eye_hue_phase: 0.0,
            eye_idle_phase: 0.0,
            cwd_arc,
            scan_rx,
            meta_req_tx,
            meta_res_rx,
            meta_pending: HashSet::new(),
            art_req_tx,
            art_res_rx,
            art_pending: HashSet::new(),
            art_missing: HashSet::new(),
            last_preview: None,
            preview_pending: None,
            queue: VecDeque::new(),
            shuffle: false,
            loop_one: false,
            rng_state: seed_rng(),
            seek_req_tx,
            seek_res_rx,
            seek_id_counter: 0,
            pending_seek: None,
            theme_idx: theme::load_theme_idx(),
            theme_open: false,
            theme_prev_idx: 0,
            theme_scroll: 0,
            viz_open: false,
            viz_prev_mode: VizMode::Off,
            viz_scroll: 0,
        })
    }

    fn apply_scan_updates(&mut self) {
        let mut latest: Option<(PathBuf, Vec<Entry>)> = None;
        while let Ok(v) = self.scan_rx.try_recv() {
            latest = Some(v);
        }
        if let Some((cwd, raw)) = latest {
            if cwd == self.cwd {
                let mut next = raw;
                if self.cwd != self.root {
                    next.insert(0, Entry::Parent);
                }
                self.update_entries(next);
            }
        }
    }

    fn update_entries(&mut self, new_entries: Vec<Entry>) {
        let sel_path = selected_entry_key(&self.entries, self.selected);
        let hov_path = self.hovered.and_then(|i| selected_entry_key(&self.entries, i));

        self.entries = new_entries;

        self.selected = sel_path
            .as_ref()
            .and_then(|k| find_entry(&self.entries, k))
            .unwrap_or(0)
            .min(self.entries.len().saturating_sub(1));

        self.hovered = hov_path.as_ref().and_then(|k| find_entry(&self.entries, k));

        let len = self.entries.len();
        if self.scroll >= len {
            self.scroll = len.saturating_sub(1);
        }
    }

    pub fn open_viz_picker(&mut self) {
        self.viz_prev_mode = self.viz_mode;
        // Rough initial centering — draw() will clamp to the actual list height.
        self.viz_scroll = self.viz_mode.index().saturating_sub(8);
        self.viz_open = true;
        if self.theme_open {
            self.close_theme_picker(false);
        }
    }

    pub fn close_viz_picker(&mut self, save: bool) {
        if !save {
            self.viz_mode = self.viz_prev_mode;
        } else if self.viz_mode != self.viz_prev_mode {
            crate::visualizer::save_viz_mode(self.viz_mode);
        }
        self.viz_open = false;
    }

    pub fn cycle_viz(&mut self, delta: i32) {
        let len = VizMode::ALL.len() as i32;
        if len == 0 {
            return;
        }
        let cur = self.viz_mode.index() as i32;
        let next = (cur + delta).clamp(0, len - 1) as usize;
        self.viz_mode = VizMode::ALL[next];
    }

    pub fn open_theme_picker(&mut self) {
        self.theme_prev_idx = self.theme_idx;
        // Rough initial centering — draw() will clamp to the actual list height.
        self.theme_scroll = self.theme_idx.saturating_sub(8);
        self.theme_open = true;
        if self.viz_open {
            self.close_viz_picker(false);
        }
    }

    pub fn close_theme_picker(&mut self, save: bool) {
        if !save {
            self.theme_idx = self.theme_prev_idx;
        } else if self.theme_idx != self.theme_prev_idx {
            theme::save_theme_idx(self.theme_idx);
        }
        self.theme_open = false;
    }

    pub fn cycle_theme(&mut self, delta: i32) {
        let len = THEMES.len() as i32;
        if len == 0 {
            return;
        }
        let next = (self.theme_idx as i32 + delta).clamp(0, len - 1) as usize;
        self.theme_idx = next;
    }

    pub fn set_hovered(&mut self, idx: Option<usize>) {
        self.hovered = idx;
    }

    /// Watch the current preview target (hover → playing → selection) and
    /// debounce-trigger background metadata + art loads when it changes.
    fn observe_preview(&mut self) {
        let target = self.preview_target();
        if self.last_preview == target {
            return;
        }
        self.last_preview = target.clone();
        self.preview_pending = target.map(|t| (t, Instant::now()));
    }

    fn fire_preview_load(&mut self) {
        let Some((target, since)) = self.preview_pending.clone() else {
            return;
        };
        if since.elapsed() < HOVER_DEBOUNCE {
            return;
        }
        if self.preview_target().as_ref() != Some(&target) {
            self.preview_pending = None;
            return;
        }
        self.preview_pending = None;

        if let Some(track) = target.track.as_ref() {
            if !self.meta_cache.contains_key(track) && !self.meta_pending.contains(track) {
                self.meta_pending.insert(track.clone());
                let _ = self.meta_req_tx.send(track.clone());
            }
        }
        if !self.image_cache.contains_key(&target.art_key)
            && !self.art_pending.contains(&target.art_key)
            && !self.art_missing.contains(&target.art_key)
        {
            self.art_pending.insert(target.art_key.clone());
            let _ = self.art_req_tx.send(ArtRequest {
                key: target.art_key,
                source: target.art_source,
            });
        }
    }

    fn drain_meta_results(&mut self) {
        while let Ok((path, meta)) = self.meta_res_rx.try_recv() {
            self.meta_pending.remove(&path);
            self.meta_cache.insert(path, meta.unwrap_or_default());
        }
    }

    fn drain_art_results(&mut self) {
        while let Ok((path, img)) = self.art_res_rx.try_recv() {
            self.art_pending.remove(&path);
            match img {
                Some(img) => {
                    if let Some(picker) = self.picker.as_ref() {
                        let proto = picker.new_resize_protocol(img);
                        self.image_cache.insert(path, proto);
                    }
                }
                None => {
                    // Remember misses so we don't re-request every hover.
                    self.art_missing.insert(path);
                }
            }
        }
    }

    pub fn preview_target(&self) -> Option<PreviewTarget> {
        if let Some(i) = self.hovered {
            match self.entries.get(i) {
                Some(Entry::Track(p)) => return Some(track_target(p)),
                Some(Entry::Folder(p)) => {
                    if let Some(first) = library::first_track(p) {
                        let label = p
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        return Some(PreviewTarget {
                            art_key: p.clone(),
                            art_source: first,
                            track: None,
                            fallback_label: label,
                        });
                    }
                }
                _ => {}
            }
        }
        if let Some(p) = &self.now_playing {
            return Some(track_target(p));
        }
        if let Some(Entry::Track(p)) = self.entries.get(self.selected) {
            return Some(track_target(p));
        }
        None
    }

    pub fn activate_index(&mut self, idx: usize) {
        self.selected = idx;
        match self.entries.get(idx).cloned() {
            Some(Entry::Parent) => self.ascend(),
            Some(Entry::Folder(p)) => self.descend(p),
            Some(Entry::Track(p)) => self.play_path(p),
            None => {}
        }
    }

    pub fn activate_selected(&mut self) {
        self.activate_index(self.selected);
    }

    fn descend(&mut self, folder: PathBuf) {
        self.navigate_to(folder);
    }

    fn ascend(&mut self) {
        if self.cwd == self.root {
            return;
        }
        if let Some(parent) = self.cwd.parent() {
            let parent = parent.to_path_buf();
            self.navigate_to(parent);
        }
    }

    fn navigate_to(&mut self, target: PathBuf) {
        let target = target.canonicalize().unwrap_or(target);
        if !target.starts_with(&self.root) {
            return;
        }
        let prev_dir = self.cwd.clone();
        self.cwd = target.clone();
        self.entries = build_entries(&self.cwd, &self.root);
        if let Ok(mut g) = self.cwd_arc.lock() {
            *g = self.cwd.clone();
        }
        self.scroll = 0;
        self.hovered = None;
        self.selected = if prev_dir != self.cwd && prev_dir.starts_with(&self.cwd) {
            // when ascending, highlight the folder we came from
            self.entries
                .iter()
                .position(|e| matches!(e, Entry::Folder(p) if p == &prev_dir))
                .unwrap_or(0)
        } else {
            // descending or sideways: skip the "../" row if present
            if matches!(self.entries.first(), Some(Entry::Parent)) {
                1.min(self.entries.len().saturating_sub(1))
            } else {
                0
            }
        };
    }

    fn play_path(&mut self, path: PathBuf) {
        match self.player.load(&path) {
            Ok(dur) => {
                self.track_duration = dur.or_else(|| metadata::fast_duration(&path));
                self.now_playing = Some(path);
                self.elapsed = Duration::ZERO;
                self.last_tick = Instant::now();
                self.status.clear();
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
                self.now_playing = None;
                self.track_duration = None;
            }
        }
    }

    pub fn enqueue_selected(&mut self) {
        self.enqueue_index(self.selected);
    }

    pub fn enqueue_index(&mut self, idx: usize) {
        if let Some(Entry::Track(p)) = self.entries.get(idx).cloned() {
            self.queue.push_back(p);
        }
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
    }

    pub fn toggle_loop(&mut self) {
        self.loop_one = !self.loop_one;
    }

    pub fn queue_position(&self, path: &Path) -> Option<usize> {
        self.queue.iter().position(|p| p == path)
    }

    fn advance(&mut self) {
        if let Some(next) = self.queue.pop_front() {
            self.play_path(next);
            return;
        }
        if let Some(next) = self.pick_next_in_folder() {
            self.play_path(next);
            return;
        }
        // nothing to play next
        self.now_playing = None;
        self.track_duration = None;
        self.elapsed = Duration::ZERO;
    }

    fn pick_next_in_folder(&mut self) -> Option<PathBuf> {
        let tracks: Vec<PathBuf> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Track(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        if tracks.is_empty() {
            return None;
        }
        let current = self.now_playing.clone();
        if self.shuffle {
            if tracks.len() == 1 {
                return Some(tracks[0].clone());
            }
            // pick a random track different from the current
            loop {
                let r = (xorshift(&mut self.rng_state) as usize) % tracks.len();
                if Some(&tracks[r]) != current.as_ref() {
                    return Some(tracks[r].clone());
                }
            }
        } else {
            let pos = current
                .as_ref()
                .and_then(|c| tracks.iter().position(|t| t == c));
            match pos {
                Some(i) if i + 1 < tracks.len() => Some(tracks[i + 1].clone()),
                Some(_) => None, // last track — stop
                None => Some(tracks[0].clone()),
            }
        }
    }

    pub fn toggle_play(&mut self) {
        self.player.toggle();
        self.last_tick = Instant::now();
    }

    pub fn adjust_volume(&mut self, delta_pct: f32) {
        let next = (self.player.volume() + delta_pct / 100.0).clamp(0.0, 1.0);
        self.player.set_volume(next);
    }

    pub fn set_volume_fraction(&mut self, frac: f32) {
        self.player.set_volume(frac.clamp(0.0, 1.0));
    }

    pub fn seek_fraction(&mut self, frac: f32) {
        let frac = frac.clamp(0.0, 1.0);
        let Some(total) = self.track_duration else {
            if self.now_playing.is_none() {
                self.status = "Nothing playing — click a track first".to_string();
            } else {
                self.status = "Track has no duration; cannot seek".to_string();
            }
            return;
        };
        let target = Duration::from_secs_f32(total.as_secs_f32() * frac);
        if self.player.try_seek(target).is_ok() {
            self.tap.clear();
            self.elapsed = target;
            self.last_tick = Instant::now();
            self.pending_seek = None;
            self.status.clear();
            return;
        }
        // Fast seek failed (format doesn't support it). Offload to worker.
        let Some(path) = self.now_playing.clone() else {
            self.status = "Nothing to seek".to_string();
            return;
        };
        self.seek_id_counter += 1;
        let id = self.seek_id_counter;
        self.pending_seek = Some((id, target));
        let req = SeekRequest {
            id,
            path,
            target,
            volume: self.player.volume(),
        };
        let _ = self.seek_req_tx.send(req);
        self.status.clear();
    }

    fn drain_seek_results(&mut self) {
        let mut chosen: Option<SeekResult> = None;
        while let Ok(r) = self.seek_res_rx.try_recv() {
            chosen = Some(r);
        }
        let Some(r) = chosen else { return };
        let Some((pending_id, pending_target)) = self.pending_seek else {
            // No outstanding seek (already superseded or fast-path won); discard.
            return;
        };
        if r.id != pending_id {
            // Stale result from an older request; ignore.
            return;
        }
        self.player.install_sink(r.sink);
        self.elapsed = pending_target;
        self.last_tick = Instant::now();
        self.pending_seek = None;
        self.status.clear();
    }


    pub fn move_selection(&mut self, delta: i32, viewport: usize) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as i32;
        let next = (self.selected as i32 + delta).clamp(0, len - 1) as usize;
        self.selected = next;
        self.ensure_visible(viewport);
    }

    pub fn ensure_visible(&mut self, viewport: usize) {
        if viewport == 0 {
            return;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + viewport {
            self.scroll = self.selected + 1 - viewport;
        }
    }

    pub fn scroll_by(&mut self, delta: i32, viewport: usize) {
        if self.entries.is_empty() {
            return;
        }
        let max = self.entries.len().saturating_sub(viewport.max(1));
        let next = (self.scroll as i32 + delta).clamp(0, max as i32) as usize;
        self.scroll = next;
    }

    pub fn ascend_action(&mut self) {
        self.ascend();
    }

    pub fn descend_selected(&mut self) {
        if let Some(Entry::Folder(p)) = self.entries.get(self.selected).cloned() {
            self.descend(p);
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        if self.player.is_playing() && self.pending_seek.is_none() {
            self.elapsed += now - self.last_tick;
            if let Some(total) = self.track_duration {
                if self.elapsed > total {
                    self.elapsed = total;
                }
            }
        }
        self.last_tick = now;
        self.apply_scan_updates();
        self.observe_preview();
        self.fire_preview_load();
        self.drain_meta_results();
        self.drain_art_results();
        self.drain_seek_results();
        if self.now_playing.is_some() && self.pending_seek.is_none() && self.player.is_finished() {
            if self.loop_one {
                if let Some(p) = self.now_playing.clone() {
                    self.play_path(p);
                }
            } else {
                self.advance();
            }
        }
    }

    pub fn handle_click(&mut self, target: ClickTarget) {
        match target {
            ClickTarget::TogglePlay => self.toggle_play(),
            ClickTarget::Quit => self.should_quit = true,
            ClickTarget::SelectIndex(i) => self.activate_index(i),
            ClickTarget::Volume => {}
            ClickTarget::SeekBar => {}
            ClickTarget::ToggleViz => self.open_viz_picker(),
            ClickTarget::ToggleShuffle => self.toggle_shuffle(),
            ClickTarget::ToggleLoop => self.toggle_loop(),
            ClickTarget::OpenThemes => self.open_theme_picker(),
            ClickTarget::ThemeRow(i) => {
                self.theme_idx = i;
                self.close_theme_picker(true);
            }
            ClickTarget::VizRow(i) => {
                if let Some(mode) = VizMode::ALL.get(i).copied() {
                    self.viz_mode = mode;
                    self.close_viz_picker(true);
                }
            }
        }
    }

    pub fn register_hit(&mut self, rect: Rect, target: ClickTarget) {
        self.hit_map.push(rect, target);
    }

    pub fn reset_hits(&mut self) {
        self.hit_map.clear();
    }
}

fn track_target(p: &Path) -> PreviewTarget {
    let key = p
        .parent()
        .map(|q| q.to_path_buf())
        .unwrap_or_else(|| p.to_path_buf());
    PreviewTarget {
        art_key: key,
        art_source: p.to_path_buf(),
        track: Some(p.to_path_buf()),
        fallback_label: String::new(),
    }
}

fn build_entries(cwd: &Path, root: &Path) -> Vec<Entry> {
    let mut entries = library::list_dir(cwd);
    if cwd != root {
        entries.insert(0, Entry::Parent);
    }
    entries
}

#[derive(Clone, PartialEq, Eq)]
enum EntryKey {
    Parent,
    Path(PathBuf),
}

fn selected_entry_key(entries: &[Entry], idx: usize) -> Option<EntryKey> {
    match entries.get(idx)? {
        Entry::Parent => Some(EntryKey::Parent),
        Entry::Folder(p) | Entry::Track(p) => Some(EntryKey::Path(p.clone())),
    }
}

fn find_entry(entries: &[Entry], key: &EntryKey) -> Option<usize> {
    entries.iter().position(|e| match (e, key) {
        (Entry::Parent, EntryKey::Parent) => true,
        (Entry::Folder(p), EntryKey::Path(k)) => p == k,
        (Entry::Track(p), EntryKey::Path(k)) => p == k,
        _ => false,
    })
}

fn seed_rng() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xC0FFEE_C0FFEE);
    nanos | 1
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x.max(1);
    *state
}

fn spawn_meta_worker() -> (
    mpsc::Sender<PathBuf>,
    mpsc::Receiver<(PathBuf, Option<TrackMeta>)>,
) {
    let (req_tx, req_rx) = mpsc::channel::<PathBuf>();
    let (res_tx, res_rx) = mpsc::channel::<(PathBuf, Option<TrackMeta>)>();
    thread::spawn(move || {
        while let Ok(path) = req_rx.recv() {
            let result = metadata::read_tags(&path).ok();
            if res_tx.send((path, result)).is_err() {
                return;
            }
        }
    });
    (req_tx, res_rx)
}

/// Multiple workers so two slow JPEG decodes don't block the queue head.
/// Workers race on a shared Mutex<Receiver>; whichever locks first takes the
/// next job. The lock is released before the (slow) decode runs.
fn spawn_art_workers(
    n: usize,
) -> (
    mpsc::Sender<ArtRequest>,
    mpsc::Receiver<(PathBuf, Option<DynamicImage>)>,
) {
    let (req_tx, req_rx) = mpsc::channel::<ArtRequest>();
    let req_rx = Arc::new(Mutex::new(req_rx));
    let (res_tx, res_rx) = mpsc::channel::<(PathBuf, Option<DynamicImage>)>();
    for _ in 0..n.max(1) {
        let rx = req_rx.clone();
        let tx = res_tx.clone();
        thread::spawn(move || loop {
            let req = {
                let lock = match rx.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                match lock.recv() {
                    Ok(r) => r,
                    Err(_) => return,
                }
            };
            let img = metadata::read_art(&req.source).ok();
            if tx.send((req.key, img)).is_err() {
                return;
            }
        });
    }
    (req_tx, res_rx)
}

fn spawn_scanner(
    cwd_arc: Arc<Mutex<PathBuf>>,
    initial: Vec<Entry>,
) -> mpsc::Receiver<(PathBuf, Vec<Entry>)> {
    let (tx, rx) = mpsc::channel();
    // Scanner sends raw list_dir output (no Parent row); App prepends it on receive.
    let initial_raw: Vec<Entry> = initial
        .into_iter()
        .filter(|e| !matches!(e, Entry::Parent))
        .collect();
    thread::spawn(move || {
        let mut last_cwd: PathBuf = cwd_arc.lock().map(|g| g.clone()).unwrap_or_default();
        let mut last = initial_raw;
        loop {
            thread::sleep(SCAN_INTERVAL);
            let cwd = match cwd_arc.lock() {
                Ok(g) => g.clone(),
                Err(_) => return,
            };
            let next = library::list_dir(&cwd);
            if cwd != last_cwd {
                last_cwd = cwd;
                last = next;
                continue;
            }
            if next != last {
                last = next.clone();
                if tx.send((cwd, next)).is_err() {
                    return;
                }
            }
        }
    });
    rx
}
