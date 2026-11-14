use std::time::Duration;

use anyhow::{Context as _, Result};
use async_trait::async_trait;
use serde::Deserialize;

use crate::lyrics::ttml;
use crate::{LyricsHit, LyricsProvider, LyricsQuery};

const SOURCE: &str = "Apple Music";
const SEARCH_LRCRED_SEARCH: &str = "https://lrc.red/search.json";
const SEARCH_LRCRED_API: &str = "https://lrc.red/api/v1";
const SEARCH_BINIMUM: &str = "https://lyrics-api.binimum.org/";
const TRUST: u32 = 180;
const AGENT: &str = concat!(
    "sonora/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/sonorahq/sonora)"
);

pub struct Binimum {
    http: reqwest::Client,
}

impl Binimum {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    async fn find(&self, query: &LyricsQuery) -> Result<Vec<Match>> {
        // 1. Try lrc.red search.json first (direct ISRC matching)
        if let Ok(results) = self.find_lrcred_search(query).await {
            if !results.is_empty() {
                return Ok(results);
            }
        }
        // 2. Try lrc.red api/v1 structured query (track & artist)
        if let Ok(results) = self.find_endpoint(SEARCH_LRCRED_API, query, false).await {
            if !results.is_empty() {
                return Ok(results);
            }
        }
        // 3. Try lrc.red api/v1 full-text query
        if let Ok(results) = self.find_endpoint(SEARCH_LRCRED_API, query, true).await {
            if !results.is_empty() {
                return Ok(results);
            }
        }
        // 4. Fallback to binimum if lrc.red didn't match
        self.find_endpoint(SEARCH_BINIMUM, query, false).await
    }

    async fn find_lrcred_search(&self, query: &LyricsQuery) -> Result<Vec<Match>> {
        let q = format!("{} {}", query.title, query.artist);
        let response = self
            .http
            .get(SEARCH_LRCRED_SEARCH)
            .query(&[("q", q.as_str())])
            .header("User-Agent", AGENT)
            .send()
            .await
            .context("cannot reach lrc.red search")?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !status.is_success() {
            anyhow::bail!("lrc.red search answered with status {status}");
        }
        let found: SearchJson = response
            .json()
            .await
            .context("cannot read lrc.red search response")?;
        if found.hits.is_empty() {
            return Ok(Vec::new());
        }

        let query_secs = query.duration.as_secs_f64();
        let mut hits = found.hits;
        if query_secs > 0.0 {
            hits.sort_by(|a, b| {
                let diff_a = (a.duration - query_secs).abs();
                let diff_b = (b.duration - query_secs).abs();
                diff_a
                    .partial_cmp(&diff_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let matches = hits
            .into_iter()
            .map(|hit| Match {
                track_name: hit.title,
                artist_name: hit.artist,
                album_name: hit.album,
                duration: hit.duration.round() as u64,
                timing_type: "word".to_owned(),
                lyrics_url: Some(format!("https://lrc.red/s/{}.ttml", hit.isrc)),
            })
            .collect();
        Ok(matches)
    }

    async fn find_endpoint(
        &self,
        endpoint: &str,
        query: &LyricsQuery,
        use_q: bool,
    ) -> Result<Vec<Match>> {
        let duration = query.duration.as_secs().to_string();
        let query_str;
        let mut params = Vec::new();
        if use_q {
            query_str = format!("{} {}", query.artist, query.title);
            params.push(("q", query_str.as_str()));
        } else {
            params.push(("track", query.title.as_str()));
            params.push(("artist", query.artist.as_str()));
            if let Some(album) = query.album.as_deref().filter(|album| !album.is_empty()) {
                params.push(("album", album));
            }
        }
        if !query.duration.is_zero() {
            params.push(("duration", duration.as_str()));
        }

        let response = self
            .http
            .get(endpoint)
            .query(&params)
            .header("User-Agent", AGENT)
            .send()
            .await
            .context("cannot reach the apple lyrics catalogue")?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !status.is_success() {
            anyhow::bail!("the apple lyrics catalogue answered with status {status}");
        }
        let found: Found = response
            .json()
            .await
            .context("cannot read the apple lyrics catalogue response")?;
        Ok(found.results)
    }

    async fn sheet(&self, url: &str) -> Result<String> {
        if !url.starts_with("https://") {
            anyhow::bail!("the apple lyrics catalogue offered an insecure sheet");
        }
        let response = self
            .http
            .get(url)
            .header("User-Agent", AGENT)
            .send()
            .await
            .context("cannot reach the apple lyrics sheet")?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("the apple lyrics sheet answered with status {status}");
        }
        response
            .text()
            .await
            .context("cannot read the apple lyrics sheet")
    }
}

impl Default for Binimum {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct SearchJson {
    #[serde(default)]
    hits: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    isrc: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
    #[serde(default)]
    duration: f64,
}

#[derive(Deserialize)]
struct Found {
    #[serde(default)]
    results: Vec<Match>,
}

#[derive(Deserialize)]
struct Match {
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    album_name: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    timing_type: String,
    #[serde(rename = "lyricsUrl")]
    lyrics_url: Option<String>,
}

impl Match {
    fn worded(&self) -> bool {
        self.timing_type == "word" || self.timing_type == "syllable"
    }
}

#[async_trait]
impl LyricsProvider for Binimum {
    fn name(&self) -> &'static str {
        SOURCE
    }

    async fn search(&self, query: &LyricsQuery) -> Result<Vec<LyricsHit>> {
        let mut found = self.find(query).await?;
        found.sort_by_key(|found| !found.worded());
        let Some((best, url)) = found
            .iter()
            .find_map(|found| Some((found, found.lyrics_url.as_deref()?)))
        else {
            return Ok(Vec::new());
        };

        let sheet = ttml::parse(&self.sheet(url).await?)?;
        if sheet.lyrics.is_empty() {
            return Ok(Vec::new());
        }

        Ok(vec![LyricsHit {
            source: SOURCE,
            trust: TRUST,
            lyrics: sheet.lyrics,
            instrumental: false,
            title: best.track_name.clone(),
            artist: best.artist_name.clone(),
            album: (!best.album_name.is_empty()).then(|| best.album_name.clone()),
            duration: (best.duration > 0).then(|| Duration::from_secs(best.duration)),
            writers: sheet.writers,
        }])
    }
}
