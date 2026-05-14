use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use image::DynamicImage;
use lofty::config::ParseOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;

#[derive(Default, Clone)]
pub struct TrackMeta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub track_no: Option<u32>,
    pub genre: Option<String>,
    pub duration_secs: Option<u64>,
}

/// Fast path: read tags + duration without parsing embedded artwork.
/// For FLAC files this skips multi-MB picture blocks entirely.
pub fn read_tags(path: &Path) -> Result<TrackMeta> {
    let tagged = Probe::open(path)?
        .options(ParseOptions::new().read_cover_art(false))
        .read()?;
    let props = tagged.properties();
    let duration_secs = Some(props.duration().as_secs());

    let mut meta = TrackMeta {
        duration_secs,
        ..Default::default()
    };

    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        meta.title = tag.title().map(|s| s.into_owned());
        meta.artist = tag.artist().map(|s| s.into_owned());
        meta.album = tag.album().map(|s| s.into_owned());
        meta.year = tag.year();
        meta.track_no = tag.track();
        meta.genre = tag.genre().map(|s| s.into_owned());
    }

    Ok(meta)
}

/// Slow path: decode embedded artwork and downscale to a sensible cap so the
/// terminal-image protocol doesn't re-resize a 3000×3000 source every frame.
pub fn read_art(path: &Path) -> Result<DynamicImage> {
    let tagged = Probe::open(path)?.read()?;
    let tag = tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .ok_or_else(|| anyhow!("no tag"))?;
    let pic = tag.pictures().first().ok_or_else(|| anyhow!("no picture"))?;
    let img = image::load_from_memory(pic.data())?;
    Ok(img.thumbnail(640, 640))
}

pub fn fast_duration(path: &Path) -> Option<Duration> {
    let tagged = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .ok()?;
    Some(tagged.properties().duration())
}
