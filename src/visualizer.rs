use std::collections::VecDeque;
use std::f32::consts::PI;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::Source;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

const TAP_CAPACITY: usize = 8192;

#[derive(Clone)]
pub struct AudioTap {
    buf: Arc<Mutex<VecDeque<f32>>>,
    sample_rate: Arc<AtomicU32>,
}

impl AudioTap {
    pub fn new() -> Self {
        Self {
            buf: Arc::new(Mutex::new(VecDeque::with_capacity(TAP_CAPACITY))),
            sample_rate: Arc::new(AtomicU32::new(44_100)),
        }
    }

    fn push(&self, sample: f32) {
        let Ok(mut b) = self.buf.lock() else {
            return;
        };
        if b.len() >= TAP_CAPACITY {
            b.pop_front();
        }
        b.push_back(sample);
    }

    pub fn snapshot(&self, n: usize) -> Vec<f32> {
        let Ok(b) = self.buf.lock() else {
            return Vec::new();
        };
        let start = b.len().saturating_sub(n);
        b.iter().skip(start).copied().collect()
    }

    pub fn clear(&self) {
        if let Ok(mut b) = self.buf.lock() {
            b.clear();
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    pub fn set_sample_rate(&self, sr: u32) {
        if sr > 0 {
            self.sample_rate.store(sr, Ordering::Relaxed);
        }
    }
}

pub struct TapSource<S> {
    inner: S,
    tap: AudioTap,
    channels: u16,
    counter: u16,
}

impl<S> TapSource<S>
where
    S: Source<Item = f32>,
{
    pub fn new(inner: S, tap: AudioTap) -> Self {
        let channels = inner.channels().max(1);
        tap.set_sample_rate(inner.sample_rate());
        Self {
            inner,
            tap,
            channels,
            counter: 0,
        }
    }
}

impl<S> Iterator for TapSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let s = self.inner.next()?;
        if self.counter == 0 {
            self.tap.push(s);
        }
        self.counter = (self.counter + 1) % self.channels;
        Some(s)
    }
}

impl<S> Source for TapSource<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.tap.clear();
        self.inner.try_seek(pos)
    }
}

pub struct Spectrum {
    planner: FftPlanner<f32>,
    size: usize,
    smoothed: Vec<f32>,
}

impl Spectrum {
    pub fn new(size: usize) -> Self {
        Self {
            planner: FftPlanner::new(),
            size,
            smoothed: Vec::new(),
        }
    }

    pub fn analyze(&mut self, samples: &[f32], bins: usize) -> Vec<f32> {
        if self.smoothed.len() != bins {
            self.smoothed = vec![0.0; bins];
        }
        if samples.len() < self.size {
            for v in &mut self.smoothed {
                *v *= 0.85;
            }
            return self.smoothed.clone();
        }

        let start = samples.len() - self.size;
        let mut buf: Vec<Complex<f32>> = (0..self.size)
            .map(|i| {
                let w = 0.5 - 0.5 * (2.0 * PI * i as f32 / (self.size - 1) as f32).cos();
                Complex::new(samples[start + i] * w, 0.0)
            })
            .collect();

        let fft = self.planner.plan_fft_forward(self.size);
        fft.process(&mut buf);

        let usable = self.size / 2;
        let mut raw = vec![0.0f32; bins];
        for i in 0..bins {
            let lo = log_index(i, bins, usable);
            let hi = log_index(i + 1, bins, usable).max(lo + 1);
            let mut sum = 0.0;
            let mut count = 0;
            for k in lo..hi.min(usable) {
                sum += buf[k].norm();
                count += 1;
            }
            raw[i] = if count > 0 { sum / count as f32 } else { 0.0 };
        }

        let mut max_mag = 0.0f32;
        for v in &raw {
            if *v > max_mag {
                max_mag = *v;
            }
        }
        let norm = if max_mag > 0.0 { 1.0 / max_mag } else { 0.0 };

        for (i, v) in raw.iter().enumerate() {
            let scaled = (v * norm).clamp(0.0, 1.0);
            let scaled = scaled.powf(0.55);
            let prev = self.smoothed[i];
            let blended = if scaled > prev {
                prev + (scaled - prev) * 0.55
            } else {
                prev * 0.78
            };
            self.smoothed[i] = blended;
        }

        self.smoothed.clone()
    }

    pub fn analyze_chroma(&mut self, samples: &[f32], sample_rate: u32) -> [f32; 12] {
        if samples.len() < self.size || sample_rate == 0 {
            return [0.0; 12];
        }
        let start = samples.len() - self.size;
        let mut buf: Vec<Complex<f32>> = (0..self.size)
            .map(|i| {
                let w = 0.5 - 0.5 * (2.0 * PI * i as f32 / (self.size - 1) as f32).cos();
                Complex::new(samples[start + i] * w, 0.0)
            })
            .collect();
        let fft = self.planner.plan_fft_forward(self.size);
        fft.process(&mut buf);

        let usable = self.size / 2;
        let sr = sample_rate as f32;
        let size = self.size as f32;
        let mut chroma = [0.0f32; 12];
        // Cover roughly A1 (~55 Hz) to C8 (~4.2 kHz).
        for k in 1..usable {
            let freq = k as f32 * sr / size;
            if freq < 55.0 || freq > 4200.0 {
                continue;
            }
            let midi = 12.0 * (freq / 440.0).log2() + 69.0;
            let pc = (midi.round() as i32).rem_euclid(12) as usize;
            chroma[pc] += buf[k].norm();
        }
        let max = chroma.iter().copied().fold(0.0f32, f32::max);
        if max > 0.0 {
            for v in &mut chroma {
                *v /= max;
            }
        }
        chroma
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub hue: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DriftPoint {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct TunnelRing {
    pub radius: f32,
    pub hue: f32,
    pub brightness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct QuakeRing {
    pub radius: f32,
    pub intensity: f32,
    pub hue: f32,
}

/// A contact ping spawned where the radar sweep meets a strong signal:
/// renders as an expanding ring centered at (x, y) in sub-pixel space.
#[derive(Clone, Copy, Debug)]
pub struct RadarBlip {
    pub x: f32,
    pub y: f32,
    pub age: f32,
    pub intensity: f32,
    pub hue: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LightningPoint {
    pub x: i32,
    pub y: i32,
    pub brightness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct HeartRipple {
    /// Implicit-curve level c: rendered as the locus where the heart equation
    /// equals this value. Grows over time so the ripple expands outward.
    pub c: f32,
    pub intensity: f32,
}

fn log_index(i: usize, bins: usize, usable: usize) -> usize {
    let frac = i as f32 / bins as f32;
    let scaled = (10f32.powf(frac * 3.0) - 1.0) / (10f32.powf(3.0) - 1.0);
    (scaled * usable as f32) as usize
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VizMode {
    Off,
    Bars,
    Wave,
    Polar,
    Chroma,
    Spark,
    Drift,
    Aurora,
    Tunnel,
    Radar,
    Mandala,
    Quake,
    Lightning,
    Julia,
    Heart,
    Eye,
}

impl VizMode {
    pub const ALL: &'static [VizMode] = &[
        VizMode::Off,
        VizMode::Bars,
        VizMode::Wave,
        VizMode::Polar,
        VizMode::Chroma,
        VizMode::Spark,
        VizMode::Drift,
        VizMode::Aurora,
        VizMode::Tunnel,
        VizMode::Radar,
        VizMode::Mandala,
        VizMode::Quake,
        VizMode::Lightning,
        VizMode::Julia,
        VizMode::Heart,
        VizMode::Eye,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn name(self) -> &'static str {
        match self {
            VizMode::Off => "Off",
            VizMode::Bars => "Bars",
            VizMode::Wave => "Wave",
            VizMode::Polar => "Polar",
            VizMode::Chroma => "Chroma",
            VizMode::Spark => "Spark",
            VizMode::Drift => "Drift",
            VizMode::Aurora => "Aurora",
            VizMode::Tunnel => "Tunnel",
            VizMode::Radar => "Radar",
            VizMode::Mandala => "Mandala",
            VizMode::Quake => "Quake",
            VizMode::Lightning => "Lightning",
            VizMode::Julia => "Julia",
            VizMode::Heart => "Heart",
            VizMode::Eye => "Eye",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            VizMode::Off => " [viz]  ",
            VizMode::Bars => " [bars] ",
            VizMode::Wave => " [wave] ",
            VizMode::Polar => " [polar]",
            VizMode::Chroma => " [chrm] ",
            VizMode::Spark => " [spark]",
            VizMode::Drift => " [drift]",
            VizMode::Aurora => " [auro] ",
            VizMode::Tunnel => " [tunl] ",
            VizMode::Radar => " [radr] ",
            VizMode::Mandala => " [mndl] ",
            VizMode::Quake => " [quake]",
            VizMode::Lightning => " [bolt] ",
            VizMode::Julia => " [julia]",
            VizMode::Heart => " [heart]",
            VizMode::Eye => " [eye]  ",
        }
    }

    pub fn is_on(self) -> bool {
        !matches!(self, VizMode::Off)
    }
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("detune").join("visualizer"))
}

pub fn save_viz_mode(mode: VizMode) {
    let Some(path) = config_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, mode.name());
}

pub fn load_viz_mode() -> VizMode {
    let Some(path) = config_path() else { return VizMode::Off };
    let Ok(contents) = std::fs::read_to_string(&path) else { return VizMode::Off };
    let name = contents.trim();
    VizMode::ALL
        .iter()
        .copied()
        .find(|m| m.name().eq_ignore_ascii_case(name))
        .unwrap_or(VizMode::Off)
}
