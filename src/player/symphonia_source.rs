use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use rodio::source::SeekError;
use rodio::Source;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder as CodecDecoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// A `MediaSource` wrapper around a `File` that **does** report `byte_len`,
/// which the FLAC (and several other) symphonia demuxers require to seek.
struct SeekableFile {
    file: File,
    length: u64,
}

impl SeekableFile {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("opening track {}", path.display()))?;
        let length = file
            .metadata()
            .with_context(|| format!("stat track {}", path.display()))?
            .len();
        Ok(Self { file, length })
    }
}

impl Read for SeekableFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }
}

impl Seek for SeekableFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos)
    }
}

impl MediaSource for SeekableFile {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.length)
    }
}

pub struct SymphoniaSource {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn CodecDecoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    total_duration: Option<Duration>,
    buffer: Vec<f32>,
    cursor: usize,
    exhausted: bool,
}

impl SymphoniaSource {
    pub fn open(path: &Path) -> Result<Self> {
        let source = SeekableFile::open(path)?;
        let mss = MediaSourceStream::new(Box::new(source), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .with_context(|| format!("probing track {}", path.display()))?;

        let reader = probed.format;

        let track = reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow!("no audio track in {}", path.display()))?;

        let track_id = track.id;
        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or_else(|| anyhow!("unknown sample rate in {}", path.display()))?;
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(2)
            .max(1);

        let total_duration = match (track.codec_params.n_frames, track.codec_params.time_base) {
            (Some(n), Some(tb)) => {
                let t = tb.calc_time(n);
                Some(Duration::new(t.seconds, (t.frac * 1_000_000_000.0) as u32))
            }
            _ => None,
        };

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .with_context(|| format!("no codec for {}", path.display()))?;

        Ok(Self {
            reader,
            decoder,
            track_id,
            sample_rate,
            channels,
            total_duration,
            buffer: Vec::new(),
            cursor: 0,
            exhausted: false,
        })
    }

    pub fn total_duration_opt(&self) -> Option<Duration> {
        self.total_duration
    }

    fn refill(&mut self) -> bool {
        loop {
            let packet = match self.reader.next_packet() {
                Ok(p) => p,
                Err(SymphError::IoError(_)) => {
                    self.exhausted = true;
                    return false;
                }
                Err(SymphError::ResetRequired) => {
                    self.exhausted = true;
                    return false;
                }
                Err(_) => {
                    self.exhausted = true;
                    return false;
                }
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let cap = decoded.capacity() as u64;
                    let mut sb = SampleBuffer::<f32>::new(cap, spec);
                    sb.copy_interleaved_ref(decoded);
                    self.buffer.clear();
                    self.buffer.extend_from_slice(sb.samples());
                    self.cursor = 0;
                    return true;
                }
                Err(SymphError::DecodeError(_)) => continue,
                Err(_) => {
                    self.exhausted = true;
                    return false;
                }
            }
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.exhausted {
            return None;
        }
        if self.cursor >= self.buffer.len() {
            if !self.refill() {
                return None;
            }
        }
        let s = self.buffer[self.cursor];
        self.cursor += 1;
        Some(s)
    }
}

impl Source for SymphoniaSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.buffer.len().saturating_sub(self.cursor))
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let time = Time::from(pos.as_secs_f64());
        self.reader
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|_| SeekError::NotSupported {
                underlying_source: "SymphoniaSource",
            })?;
        self.buffer.clear();
        self.cursor = 0;
        self.exhausted = false;
        Ok(())
    }
}
