//! Public playlist extraction for importing into Oto hybrid playlists.
//! Supports public playlists from Spotify, YouTube Music, Apple Music, and Deezer.

use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use reqwest::header::USER_AGENT;
use serde_json::Value;

use crate::link;

const BROWSER_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalTrack {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalPlaylist {
    pub title: String,
    pub cover_url: Option<String>,
    pub tracks: Vec<ExternalTrack>,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default()
    })
}

/// Identifies the service of an external playlist link and parses its contents.
pub async fn fetch_playlist(link_text: &str) -> Result<ExternalPlaylist> {
    let mut link_text = link_text.trim();
    let followed;
    if (link_text.contains("link.") || link_text.contains("page.link") || link_text.contains("spotify.link"))
        && let Some(f) = link::follow(link_text).await
    {
        followed = f.address;
        link_text = &followed;
    }

    if link_text.starts_with("spotify:playlist:") || link_text.contains("open.spotify.com/playlist") || link_text.contains("open.spotify.com/") && link_text.contains("/playlist/") {
        return fetch_spotify(link_text).await;
    }

    if link_text.contains("deezer.com/") && link_text.contains("playlist") {
        return fetch_deezer(link_text).await;
    }

    if link_text.contains("music.apple.com/") && link_text.contains("playlist") {
        return fetch_apple(link_text).await;
    }

    if (link_text.contains("youtube.com/") || link_text.contains("youtu.be/")) && (link_text.contains("list=") || link_text.contains("/playlist")) {
        return fetch_youtube(link_text).await;
    }

    bail!("unsupported playlist link format. Supported: Spotify, YouTube Music, Apple Music, Deezer")
}

async fn fetch_spotify(link: &str) -> Result<ExternalPlaylist> {
    let id = extract_spotify_id(link)
        .with_context(|| format!("cannot extract Spotify playlist ID from {link}"))?;

    let embed_url = format!("https://open.spotify.com/embed/playlist/{id}");
    let response = client()
        .get(&embed_url)
        .header(USER_AGENT, BROWSER_UA)
        .send()
        .await
        .with_context(|| format!("cannot fetch Spotify playlist {id}"))?;

    let html = response.text().await.context("cannot read Spotify embed page")?;
    let start_tag = "<script id=\"__NEXT_DATA__\" type=\"application/json\">";
    let start = html.find(start_tag)
        .with_context(|| "Spotify playlist metadata not found on page")?
        + start_tag.len();
    let end = html[start..].find("</script>")
        .map(|idx| start + idx)
        .with_context(|| "malformed Spotify metadata tag")?;

    let json: Value = serde_json::from_str(&html[start..end])
        .context("cannot parse Spotify JSON metadata")?;

    let entity = &json["props"]["pageProps"]["state"]["data"]["entity"];
    let title = entity["title"]
        .as_str()
        .or_else(|| entity["name"].as_str())
        .unwrap_or("Spotify Playlist")
        .to_string();

    let cover_url = entity["coverArt"]["sources"]
        .as_array()
        .and_then(|sources| sources.first())
        .and_then(|source| source["url"].as_str())
        .map(String::from);

    let mut tracks = Vec::new();
    if let Some(list) = entity["trackList"].as_array() {
        for item in list {
            let Some(title) = item["title"].as_str() else { continue };
            let artist = item["subtitle"].as_str().unwrap_or("").to_string();
            let duration = item["duration"].as_u64().map(Duration::from_millis);
            tracks.push(ExternalTrack {
                title: title.to_string(),
                artist,
                album: None,
                duration,
            });
        }
    }

    Ok(ExternalPlaylist {
        title,
        cover_url,
        tracks,
    })
}

fn extract_spotify_id(link: &str) -> Option<String> {
    if let Some(id) = link.strip_prefix("spotify:playlist:") {
        return Some(id.split('?').next().unwrap_or(id).to_string());
    }
    let url = link::address(link)?;
    let segments = link::segments(&url);
    let pos = segments.iter().position(|&s| s == "playlist")?;
    segments.get(pos + 1).map(|id| id.split('?').next().unwrap_or(id).to_string())
}

async fn fetch_deezer(link: &str) -> Result<ExternalPlaylist> {
    let id = extract_deezer_id(link)
        .with_context(|| format!("cannot extract Deezer playlist ID from {link}"))?;

    let api_url = format!("https://api.deezer.com/playlist/{id}");
    let response = client()
        .get(&api_url)
        .header(USER_AGENT, BROWSER_UA)
        .send()
        .await
        .with_context(|| format!("cannot fetch Deezer playlist {id}"))?;

    let json: Value = response.json().await.context("cannot parse Deezer JSON")?;
    if let Some(err) = json.get("error") {
        bail!("Deezer API error: {}", err["message"].as_str().unwrap_or("unknown error"));
    }

    let title = json["title"].as_str().unwrap_or("Deezer Playlist").to_string();
    let cover_url = json["picture_medium"]
        .as_str()
        .or_else(|| json["picture_big"].as_str())
        .map(String::from);

    let mut tracks = Vec::new();
    if let Some(list) = json["tracks"]["data"].as_array() {
        for item in list {
            let Some(title) = item["title"].as_str() else { continue };
            let artist = item["artist"]["name"].as_str().unwrap_or("").to_string();
            let album = item["album"]["title"].as_str().map(String::from);
            let duration = item["duration"].as_u64().map(Duration::from_secs);
            tracks.push(ExternalTrack {
                title: title.to_string(),
                artist,
                album,
                duration,
            });
        }
    }

    Ok(ExternalPlaylist {
        title,
        cover_url,
        tracks,
    })
}

fn extract_deezer_id(link: &str) -> Option<String> {
    let url = link::address(link)?;
    let segments = link::segments(&url);
    let pos = segments.iter().position(|&s| s == "playlist")?;
    segments.get(pos + 1).map(|id| id.split('?').next().unwrap_or(id).to_string())
}

async fn fetch_apple(link: &str) -> Result<ExternalPlaylist> {
    let response = client()
        .get(link)
        .header(USER_AGENT, BROWSER_UA)
        .send()
        .await
        .with_context(|| format!("cannot fetch Apple Music playlist {link}"))?;

    let html = response.text().await.context("cannot read Apple Music page")?;
    let start_tag = "<script type=\"application/json\" id=\"serialized-server-data\">";
    let start = html.find(start_tag)
        .with_context(|| "Apple Music metadata not found on page")?
        + start_tag.len();
    let end = html[start..].find("</script>")
        .map(|idx| start + idx)
        .with_context(|| "malformed Apple Music metadata tag")?;

    let json: Value = serde_json::from_str(&html[start..end])
        .context("cannot parse Apple Music JSON metadata")?;

    let mut title = "Apple Music Playlist".to_string();
    let mut cover_url = None;
    let mut tracks = Vec::new();

    if let Some(sections) = json["data"][0]["data"]["sections"].as_array() {
        for section in sections {
            if let Some(items) = section["items"].as_array() {
                for item in items {
                    if let Some(t) = item["title"].as_str()
                        && item.get("artistName").is_none()
                        && title == "Apple Music Playlist"
                    {
                        title = t.to_string();
                        continue;
                    }
                    if let (Some(title), Some(artist)) = (item["title"].as_str(), item["artistName"].as_str()) {
                        let duration = item["duration"].as_u64().map(Duration::from_millis);
                        tracks.push(ExternalTrack {
                            title: title.to_string(),
                            artist: artist.to_string(),
                            album: None,
                            duration,
                        });
                    }
                }
            }
        }
    }

    if let Some(artwork) = json["data"][0]["data"]["seoData"]["artworkUrl"].as_str() {
        cover_url = Some(artwork.to_string());
    }

    Ok(ExternalPlaylist {
        title,
        cover_url,
        tracks,
    })
}

async fn fetch_youtube(link: &str) -> Result<ExternalPlaylist> {
    let url = link::address(link).with_context(|| format!("invalid URL {link}"))?;
    let list_id = link::query(&url, "list")
        .or_else(|| {
            let segments = link::segments(&url);
            let pos = segments.iter().position(|&s| s == "playlist")?;
            segments.get(pos + 1).map(|id| id.split('?').next().unwrap_or(id).to_string())
        })
        .with_context(|| "missing list parameter in YouTube playlist link")?;

    let yt = ytmusic::YtMusic::anonymous();
    let detail = yt.playlist_page(&list_id).await
        .with_context(|| format!("cannot fetch YouTube playlist {list_id}"))?;

    let title = detail.playlist.title;
    let cover_url = detail.playlist.thumbnails.first().map(|t| t.url.clone());
    let mut tracks = Vec::new();

    for track in detail.tracks {
        let artist = track.artists.first().map(|a| a.name.clone()).unwrap_or_default();
        let album = track.album.map(|a| a.name);
        let duration = track.duration;
        tracks.push(ExternalTrack {
            title: track.title,
            artist,
            album,
            duration,
        });
    }

    Ok(ExternalPlaylist {
        title,
        cover_url,
        tracks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_spotify_id() {
        assert_eq!(
            extract_spotify_id("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=123").as_deref(),
            Some("37i9dQZF1DXcBWIGoYBM5M")
        );
        assert_eq!(
            extract_spotify_id("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M").as_deref(),
            Some("37i9dQZF1DXcBWIGoYBM5M")
        );
    }

    #[test]
    fn test_extract_deezer_id() {
        assert_eq!(
            extract_deezer_id("https://www.deezer.com/playlist/3155776842?utm_source=...").as_deref(),
            Some("3155776842")
        );
    }
}
