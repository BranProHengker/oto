use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use async_trait::async_trait;

use crate::lyrics::{LOCAL, lrc};
use crate::{Lyrics, LyricsHit, LyricsProvider, LyricsQuery};

use super::{tags, wire};

/// Wins a tie against most services without outranking a better kind of sheet.
const TRUST: u32 = 100;
/// Explicit sidecar lyrics (.ttml or .lrc) placed by the user take top priority.
const TRUST_SIDECAR: u32 = 200;

/// Extensions a lyrics file beside the track may carry, in the order they are tried. TTML comes
/// first since it can time every word. Linux filesystems tell the cases apart.
const SIDECARS: [&str; 4] = ["ttml", "TTML", "lrc", "LRC"];

/// Returns a sidecar lyrics file beside the track if one exists on disk.
pub fn sidecar_file(path: &Path) -> Option<PathBuf> {
    SIDECARS.iter().find_map(|extension| {
        let sidecar_path = path.with_extension(extension);
        if sidecar_path.is_file() {
            Some(sidecar_path)
        } else {
            None
        }
    })
}

/// Reads the lyrics of a local file: a `.ttml` or `.lrc` file beside it, else what its own tags
/// carry.
/// When streaming, falls back to searching the local library index for a matching sidecar.
pub struct LocalLyrics;

#[async_trait]
impl LyricsProvider for LocalLyrics {
    fn name(&self) -> &'static str {
        LOCAL
    }

    async fn search(&self, query: &LyricsQuery) -> Result<Vec<LyricsHit>> {
        let path = query
            .track
            .as_ref()
            .and_then(|track| wire::path_from_track_id(&track.id))
            .map(PathBuf::from)
            .or_else(|| find_local_track(query));

        let Some(path) = path else {
            return Ok(Vec::new());
        };
        let res = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle
                .spawn_blocking(move || read_with_kind(&path))
                .await
                .context("local lyrics task panicked")??
        } else {
            read_with_kind(&path)?
        };
        let Some((lyrics, is_sidecar)) = res else {
            return Ok(Vec::new());
        };
        let trust = if is_sidecar { TRUST_SIDECAR } else { TRUST };
        Ok(vec![LyricsHit {
            source: LOCAL,
            trust,
            lyrics,
            instrumental: false,
            title: query.title.clone(),
            artist: query.artist.clone(),
            album: query.album.clone(),
            duration: Some(query.duration),
            writers: Vec::new(),
        }])
    }

    fn revision(&self, query: &LyricsQuery) -> Option<u64> {
        let path = query
            .track
            .as_ref()
            .and_then(|track| wire::path_from_track_id(&track.id))
            .map(PathBuf::from)
            .or_else(|| find_local_track(query))?;
        let target = sidecar_file(&path).unwrap_or(path);
        let mtime = fs::metadata(&target).ok()?.modified().ok()?;
        let duration = mtime.duration_since(std::time::UNIX_EPOCH).ok()?;
        Some(duration.as_nanos() as u64)
    }
}

/// Searches the local library index for a track matching the query's title and artist that has a
/// sidecar lyrics file (`.ttml` or `.lrc`).
fn find_local_track(query: &LyricsQuery) -> Option<PathBuf> {
    if query.title.is_empty() {
        return None;
    }
    let cache = storage::Cache::standard();
    let connection = cache.open().ok()?;
    let mut stmt = connection
        .prepare("SELECT path, track FROM local_files")
        .ok()?;
    let rows = stmt
        .query_map([], |row| {
            let path: String = row.get(0)?;
            let track: String = row.get(1)?;
            Ok((path, track))
        })
        .ok()?;

    for (path_str, track_json) in rows.flatten() {
        let path = PathBuf::from(&path_str);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        let mut title_matches = crate::lyrics::titles_alike(stem, &query.title)
            || stem.to_lowercase().contains(&query.title.to_lowercase());

        let mut artist_matches = query.artist.is_empty()
            || crate::lyrics::artists_alike(stem, &query.artist)
            || path_str
                .to_lowercase()
                .contains(&query.artist.to_lowercase());

        if let Ok(tagged) = serde_json::from_str::<wire::Tagged>(&track_json) {
            if crate::lyrics::titles_alike(&tagged.track.name, &query.title) {
                title_matches = true;
            }
            if !query.artist.is_empty()
                && (crate::lyrics::artists_alike(&tagged.track.artists, &query.artist)
                    || tagged
                        .track
                        .artists
                        .to_lowercase()
                        .contains(&query.artist.to_lowercase()))
            {
                artist_matches = true;
            }
        }

        if title_matches && artist_matches && sidecar(&path).is_some() {
            return Some(path);
        }
    }

    let dirs = [dirs::audio_dir(), dirs::download_dir()];
    for dir in dirs.into_iter().flatten() {
        let candidates = [
            format!("{} - {}.lrc", query.artist, query.title),
            format!("{}.lrc", query.title),
        ];
        for candidate in candidates {
            let safe = candidate.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
            let p = dir.join(safe);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    None
}

/// A lyrics file of the same name wins, since someone put it there for this track; the tags are
/// the fallback when there is none, or when it holds no line that decodes.
#[cfg(test)]
fn read(path: &Path) -> Result<Option<Lyrics>> {
    read_with_kind(path).map(|opt| opt.map(|(lyrics, _)| lyrics))
}

fn read_with_kind(path: &Path) -> Result<Option<(Lyrics, bool)>> {
    match sidecar(path) {
        Some(lyrics) => Ok(Some((lyrics, true))),
        None => tags::lyrics(path).map(|opt| opt.map(|lyrics| (lyrics, false))),
    }
}

/// Lyrics from `Song.ttml` or `Song.lrc` beside `Song.flac`. A file that cannot be read or has no
/// valid line is logged and skipped rather than failing the lookup, so the next file and then the
/// tags still get their turn.
fn sidecar(path: &Path) -> Option<Lyrics> {
    SIDECARS.iter().find_map(|extension| {
        let sidecar_path = path.with_extension(extension);
        match fs::read_to_string(&sidecar_path) {
            Ok(text) if extension.eq_ignore_ascii_case("ttml") => {
                tags::ttml_lyrics(&sidecar_path, &text)
            }
            Ok(text) => lrc_lyrics(&sidecar_path, &text),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => {
                log::warn!("lyrics: cannot read {}: {error}", sidecar_path.display());
                None
            }
        }
    })
}

/// Timed lyrics from an `.lrc` file, or none when it holds no valid line.
fn lrc_lyrics(lrc_path: &Path, text: &str) -> Option<Lyrics> {
    let lines = lrc::parse(text.trim_start_matches('\u{feff}'));
    if lines.is_empty() {
        if !text.trim().is_empty() {
            return Some(Lyrics::Plain {
                text: text.to_string(),
                romanized: None,
            });
        }
        log::warn!("lyrics: {} holds no valid LRC line", lrc_path.display());
        return None;
    }
    Some(Lyrics::Synced {
        lines: lines.into(),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::local::tags::tests::flac;

    const TIMED: &str = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" xml:lang="en"><head><metadata><ttm:agent type="person" xml:id="v1"/></metadata></head><body dur="0:10.000"><div begin="1.000" end="6.000"><p begin="1.000" end="3.500" ttm:agent="v1">So here I am</p><p begin="3.500" end="6.000" ttm:agent="v1">Doing it again</p></div></body></tt>"#;

    fn track(name: &str, comments: &[&str]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Song.flac");
        flac(&path, comments);
        path
    }

    fn assert_timed(lyrics: Option<Lyrics>) {
        let Some(Lyrics::Synced { lines }) = lyrics else {
            panic!("expected timed lyrics, got {lyrics:?}");
        };
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "So here I am");
        assert_eq!(lines[0].start, Duration::from_millis(1000));
        assert_eq!(lines[1].text, "Doing it again");
        assert_eq!(lines[1].start, Duration::from_millis(3500));
    }

    #[test]
    fn a_ttml_file_beside_the_track_is_decoded() {
        let path = track("sonora-local-ttml-sidecar", &[]);
        fs::write(path.with_extension("ttml"), TIMED).unwrap();

        assert_timed(read(&path).unwrap());
    }

    #[test]
    fn ttml_in_the_lyrics_tag_is_decoded() {
        let comment = format!("LYRICS={TIMED}");
        let path = track("sonora-local-ttml-tag", &[&comment]);

        assert_timed(read(&path).unwrap());
    }

    #[test]
    fn a_broken_ttml_file_falls_back_to_the_tags() {
        let path = track("sonora-local-ttml-broken", &["LYRICS=From the tags"]);
        fs::write(path.with_extension("ttml"), "<tt><body><p begin=\"1.0\">").unwrap();

        let Some(Lyrics::Plain { text, .. }) = read(&path).unwrap() else {
            panic!("expected the plain lyrics of the tags");
        };
        assert_eq!(text, "From the tags");
    }

    #[test]
    fn a_ttml_file_without_lines_gives_nothing() {
        let path = track("sonora-local-ttml-empty", &[]);
        fs::write(path.with_extension("ttml"), "<tt><body></body></tt>").unwrap();

        assert!(read(&path).unwrap().is_none());
    }
}
