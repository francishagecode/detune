use std::path::{Path, PathBuf};

const AUDIO_EXTS: &[&str] = &["mp3", "flac", "ogg", "oga", "wav", "m4a", "aac", "opus"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Parent,
    Folder(PathBuf),
    Track(PathBuf),
}

pub fn list_dir(dir: &Path) -> Vec<Entry> {
    let mut folders: Vec<PathBuf> = Vec::new();
    let mut tracks: Vec<PathBuf> = Vec::new();

    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(ft) = ent.file_type() else { continue };
        if ft.is_dir() {
            folders.push(path);
        } else if ft.is_file() && is_audio(&path) {
            tracks.push(path);
        }
    }

    folders.sort();
    tracks.sort();

    let mut out: Vec<Entry> = Vec::with_capacity(folders.len() + tracks.len());
    out.extend(folders.into_iter().map(Entry::Folder));
    out.extend(tracks.into_iter().map(Entry::Track));
    out
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.iter().any(|a| a.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Depth-first search for the first audio track inside `dir`. Used for folder
/// hover previews — bounded so an unlucky tree doesn't stall the UI thread.
pub fn first_track(dir: &Path) -> Option<PathBuf> {
    first_track_inner(dir, 0)
}

fn first_track_inner(dir: &Path, depth: usize) -> Option<PathBuf> {
    const MAX_DEPTH: usize = 4;
    let entries = list_dir(dir);
    for e in &entries {
        if let Entry::Track(p) = e {
            return Some(p.clone());
        }
    }
    if depth >= MAX_DEPTH {
        return None;
    }
    for e in &entries {
        if let Entry::Folder(p) = e {
            if let Some(t) = first_track_inner(p, depth + 1) {
                return Some(t);
            }
        }
    }
    None
}

pub fn entry_label(entry: &Entry) -> String {
    match entry {
        Entry::Parent => "../".to_string(),
        Entry::Folder(p) => {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("(folder)");
            format!("{}/", name)
        }
        Entry::Track(p) => p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)")
            .to_string(),
    }
}

