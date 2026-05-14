use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};

use crate::player::symphonia_source::SymphoniaSource;
use crate::visualizer::{AudioTap, TapSource};

pub struct SeekRequest {
    pub id: u64,
    pub path: PathBuf,
    pub target: Duration,
    pub volume: f32,
}

pub struct SeekResult {
    pub id: u64,
    pub sink: Sink,
}

pub struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Sink,
    volume: f32,
    tap: AudioTap,
}

impl Player {
    pub fn new() -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().context("opening default audio output")?;
        let sink = Sink::try_new(&handle).context("creating audio sink")?;
        sink.pause();
        let volume = 0.8;
        sink.set_volume(volume);
        Ok(Self {
            _stream: stream,
            handle,
            sink,
            volume,
            tap: AudioTap::new(),
        })
    }

    pub fn tap(&self) -> AudioTap {
        self.tap.clone()
    }

    pub fn handle(&self) -> OutputStreamHandle {
        self.handle.clone()
    }

    pub fn install_sink(&mut self, sink: Sink) {
        sink.set_volume(self.volume);
        let old = std::mem::replace(&mut self.sink, sink);
        old.stop();
        self.tap.clear();
        self.sink.play();
    }

    pub fn load(&mut self, path: &Path) -> Result<Option<Duration>> {
        let source = SymphoniaSource::open(path)?;
        let duration = source.total_duration_opt();

        let sink = Sink::try_new(&self.handle).context("creating audio sink")?;
        sink.set_volume(self.volume);
        self.tap.clear();
        let tapped = TapSource::new(source, self.tap.clone());
        sink.append(tapped);
        sink.play();

        let old = std::mem::replace(&mut self.sink, sink);
        old.stop();

        Ok(duration)
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
        self.sink.set_volume(self.volume);
    }

    pub fn try_seek(&mut self, pos: Duration) -> Result<()> {
        self.sink
            .try_seek(pos)
            .map_err(|e| anyhow::anyhow!("seek failed: {e}"))
    }

    pub fn toggle(&mut self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    pub fn is_playing(&self) -> bool {
        !self.sink.is_paused() && !self.sink.empty()
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }
}

pub fn spawn_seek_worker(
    handle: OutputStreamHandle,
    tap: AudioTap,
) -> (mpsc::Sender<SeekRequest>, mpsc::Receiver<SeekResult>) {
    let (req_tx, req_rx) = mpsc::channel::<SeekRequest>();
    let (res_tx, res_rx) = mpsc::channel::<SeekResult>();
    thread::spawn(move || {
        while let Ok(req) = req_rx.recv() {
            // Drain any newer requests already queued; only honour the latest.
            let mut latest = req;
            while let Ok(newer) = req_rx.try_recv() {
                latest = newer;
            }
            match prepare_seeked_sink(&handle, &tap, &latest) {
                Ok(sink) => {
                    let _ = res_tx.send(SeekResult {
                        id: latest.id,
                        sink,
                    });
                }
                Err(_) => {
                    // silently drop; UI keeps the old sink playing
                }
            }
        }
    });
    (req_tx, res_rx)
}

fn prepare_seeked_sink(
    handle: &OutputStreamHandle,
    tap: &AudioTap,
    req: &SeekRequest,
) -> Result<Sink> {
    let mut source = SymphoniaSource::open(&req.path)?;

    // Try native (instant) seek first via symphonia's format reader.
    if source.try_seek(req.target).is_err() {
        // Fallback: skip samples manually on the bare (un-tapped) iterator so
        // the visualizer doesn't show pre-seek audio during the skip.
        let samples_per_sec = source.sample_rate() as u64 * source.channels() as u64;
        let mut to_skip =
            (req.target.as_secs_f64() * samples_per_sec as f64).max(0.0) as u64;
        while to_skip > 0 {
            if source.next().is_none() {
                break;
            }
            to_skip -= 1;
        }
    }

    let tapped = TapSource::new(source, tap.clone());
    let sink = Sink::try_new(handle).context("creating audio sink")?;
    sink.set_volume(req.volume);
    sink.pause();
    sink.append(tapped);
    Ok(sink)
}
