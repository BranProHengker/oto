use std::fs;
use std::path::{Path, PathBuf};

use gpui::prelude::*;
use gpui::{
    AnyElement, Context, ElementId, Entity, FontWeight, Window, div, px, rgb,
};
use i18n::t;
use serde::{Deserialize, Serialize};
use ui::{ActiveTheme as _, Button, Input, TabBar};

use super::SettingsView;

const API_SEARCH: &str = "https://studio.avttr.my.id/api/tools/lrc-search";
const API_ROMAJI: &str = "https://studio.avttr.my.id/api/tools/lrc-romaji";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum LrcMode {
    #[default]
    Dual,
    Romaji,
    Original,
    Edit,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub(super) struct LrcSearchResponse {
    pub success: bool,
    pub data: Option<Vec<LrcTrackItem>>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub(super) struct LrcTrackItem {
    pub id: i64,
    #[serde(rename = "trackName")]
    pub track_name: Option<String>,
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    #[serde(rename = "albumName")]
    pub album_name: Option<String>,
    pub duration: Option<f64>,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    pub synced_lyrics: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct RomajiRequest {
    lyrics: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub(super) struct LrcRomajiResponse {
    pub success: bool,
    #[serde(rename = "isJapanese")]
    pub is_japanese: Option<bool>,
    #[serde(rename = "romajiLrc")]
    pub romaji_lrc: Option<String>,
    #[serde(rename = "dualLrc")]
    pub dual_lrc: Option<String>,
    pub lines: Option<Vec<LrcLineItem>>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub(super) struct LrcLineItem {
    pub time: Option<String>,
    pub seconds: Option<f64>,
    pub original: Option<String>,
    pub romaji: Option<String>,
}

pub(super) struct LrcStudioState {
    pub search_input: Entity<Input>,
    pub edit_input: Entity<Input>,
    pub results: Vec<LrcTrackItem>,
    pub selected_track: Option<LrcTrackItem>,
    pub original_lrc: String,
    pub romaji_lrc: String,
    pub dual_lrc: String,
    pub lines: Vec<LrcLineItem>,
    pub mode: LrcMode,
    pub is_searching: bool,
    pub is_converting: bool,
    pub status: Option<(String, bool)>,
    pub detected_title: Option<String>,
    pub detected_artist: Option<String>,
    pub detected_local_path: Option<PathBuf>,
}

impl LrcStudioState {
    pub(super) fn new(cx: &mut Context<SettingsView>) -> Self {
        let search_input = cx.new(|cx| {
            Input::new("lrc-studio-search-placeholder", cx)
                .icon("icons/search.svg")
                .clearable()
        });
        let edit_input = cx.new(|cx| {
            Input::new("lrc-studio-edit-placeholder", cx)
        });

        Self {
            search_input,
            edit_input,
            results: Vec::new(),
            selected_track: None,
            original_lrc: String::new(),
            romaji_lrc: String::new(),
            dual_lrc: String::new(),
            lines: Vec::new(),
            mode: LrcMode::Dual,
            is_searching: false,
            is_converting: false,
            status: None,
            detected_title: None,
            detected_artist: None,
            detected_local_path: None,
        }
    }
}

impl SettingsView {
    pub(super) fn lrc_studio_view(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = *cx.theme();
        self.detect_current_song(cx);

        let search_input = self.lrc_studio.search_input.clone();
        let edit_input = self.lrc_studio.edit_input.clone();
        let is_searching = self.lrc_studio.is_searching;
        let is_converting = self.lrc_studio.is_converting;
        let mode = self.lrc_studio.mode;
        let status = self.lrc_studio.status.clone();
        let has_lyrics = !self.lrc_studio.original_lrc.is_empty();
        let has_local_path = self.lrc_studio.detected_local_path.is_some();
        let local_path_display = self
            .lrc_studio
            .detected_local_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        let detected_display = match (
            self.lrc_studio.detected_title.as_deref(),
            self.lrc_studio.detected_artist.as_deref(),
        ) {
            (Some(title), Some(artist)) => format!("{artist} - {title}"),
            (Some(title), None) => title.to_string(),
            _ => "No song currently playing".to_string(),
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .child(
                // Header info card
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_4()
                    .rounded_xl()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child("Now Playing Detection"),
                                    ),
                            )
                            .child(
                                Button::new("detect-song-btn")
                                    .label(t!("lrc-studio-detect-button"))
                                    .icon("icons/radio.svg")
                                    .small()
                                    .ghost()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.trigger_detect_and_search(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("🎵 {}", detected_display)),
                    )
                    .when_some(local_path_display, |card, path| {
                        card.child(
                            div()
                                .text_xs()
                                .text_color(theme.primary)
                                .child(format!("📁 Target: {}.lrc", path.trim_end_matches(".mp3").trim_end_matches(".flac").trim_end_matches(".m4a"))),
                        )
                    }),
            )
            .child(
                // Search bar and action
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .child(div().flex_1().child(search_input))
                    .child(
                        Button::new("lrc-search-btn")
                            .label(if is_searching { "Searching..." } else { "Search" })
                            .icon("icons/search.svg")
                            .small()
                            .selected(!is_searching)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.trigger_search(cx);
                            })),
                    ),
            )
            .when_some(status, |this_div, (msg, is_error)| {
                this_div.child(
                    div()
                        .p_2()
                        .rounded_lg()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .bg(if is_error { rgb(0x451212) } else { rgb(0x12351b) })
                        .text_color(if is_error { rgb(0xffb4b4) } else { rgb(0xa3f7bf) })
                        .border_1()
                        .border_color(if is_error { rgb(0x732222) } else { rgb(0x1e5a30) })
                        .child(msg),
                )
            })
            .when(!self.lrc_studio.results.is_empty(), |this_div| {
                let results = self.lrc_studio.results.clone();
                this_div.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .rounded_xl()
                        .bg(theme.secondary)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.muted_foreground)
                                .child(format!("Search Results ({} tracks found)", results.len())),
                        )
                        .child(
                            div()
                                .id("lrc-search-results-list")
                                .flex()
                                .flex_col()
                                .gap_1()
                                .max_h(px(220.))
                                .overflow_y_scroll()
                                .children(results.into_iter().enumerate().map(|(idx, item)| {
                                    let track_name = item.track_name.clone().unwrap_or_else(|| "Unknown".into());
                                    let artist_name = item.artist_name.clone().unwrap_or_else(|| "Unknown".into());
                                    let has_synced = item.synced_lyrics.is_some();
                                    let duration = item.duration.map(|d| format!("{}:{:02}", (d as u64) / 60, (d as u64) % 60)).unwrap_or_default();
                                    let is_selected = self.lrc_studio.selected_track.as_ref().map(|t| t.id) == Some(item.id);

                                    div()
                                        .id(ElementId::NamedInteger("lrc-res".into(), idx as u64))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .p_2()
                                        .rounded_lg()
                                        .cursor_pointer()
                                        .bg(if is_selected { theme.selection } else { theme.secondary })
                                        .hover(|s| s.bg(theme.secondary_hover))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.select_track(item.clone(), cx);
                                        }))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme.foreground)
                                                        .child(track_name),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child(format!("{artist_name} • {duration}")),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_md()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .bg(if has_synced { rgb(0x13381e) } else { rgb(0x382c13) })
                                                .text_color(if has_synced { rgb(0x73e294) } else { rgb(0xe2be73) })
                                                .child(if has_synced { "SYNCED" } else { "PLAIN" }),
                                        )
                                })),
                        ),
                )
            })
            .when(has_lyrics, |this_div| {
                this_div.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .rounded_xl()
                        .bg(theme.secondary)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            // Action bar: Mode tabs & Convert / Save / Download buttons
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .flex_wrap()
                                .gap_2()
                                .child(
                                    TabBar::new("lrc-modes")
                                        .items([
                                            Button::new("mode-dual")
                                                .label("Dual-Line")
                                                .small()
                                                .ghost()
                                                .selected(mode == LrcMode::Dual)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.set_lrc_mode(LrcMode::Dual, cx);
                                                })),
                                            Button::new("mode-romaji")
                                                .label("Romaji")
                                                .small()
                                                .ghost()
                                                .selected(mode == LrcMode::Romaji)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.set_lrc_mode(LrcMode::Romaji, cx);
                                                })),
                                            Button::new("mode-original")
                                                .label("Original")
                                                .small()
                                                .ghost()
                                                .selected(mode == LrcMode::Original)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.set_lrc_mode(LrcMode::Original, cx);
                                                })),
                                            Button::new("mode-edit")
                                                .label("Edit LRC")
                                                .small()
                                                .ghost()
                                                .selected(mode == LrcMode::Edit)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.set_lrc_mode(LrcMode::Edit, cx);
                                                })),
                                        ]),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new("btn-convert-romaji")
                                                .label(if is_converting { "Converting..." } else { "Convert to Romaji" })
                                                .icon("icons/palette.svg")
                                                .small()
                                                .ghost()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.trigger_convert_romaji(cx);
                                                })),
                                        )
                                        .when(has_local_path, |btn_group| {
                                            btn_group.child(
                                                Button::new("btn-save-beside")
                                                    .label(t!("lrc-studio-save-local"))
                                                    .icon("icons/download.svg")
                                                    .small()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.save_beside_song(cx);
                                                    })),
                                            )
                                        })
                                        .child(
                                            Button::new("btn-download-lrc")
                                                .label(t!("lrc-studio-download"))
                                                .icon("icons/file-music.svg")
                                                .small()
                                                .ghost()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.download_lrc_file(cx);
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            // Lyrics Viewer / Editor
                            if mode == LrcMode::Edit {
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Editable LRC Content:"),
                                    )
                                    .child(div().w_full().child(edit_input))
                                    .into_any_element()
                            } else {
                                let lines = self.render_active_lines(&theme);
                                div()
                                    .id("lrc-preview-lines-list")
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .max_h(px(400.))
                                    .overflow_y_scroll()
                                    .p_3()
                                    .rounded_lg()
                                    .bg(theme.background)
                                    .border_1()
                                    .border_color(theme.border)
                                    .children(lines)
                                    .into_any_element()
                            },
                        ),
                )
            })
            .into_any_element()
    }

    fn render_active_lines(&self, theme: &ui::Theme) -> Vec<AnyElement> {
        let mode = self.lrc_studio.mode;

        if !self.lrc_studio.lines.is_empty() && (mode == LrcMode::Dual || mode == LrcMode::Romaji) {
            self.lrc_studio
                .lines
                .iter()
                .enumerate()
                .map(|(idx, line)| {
                    let time = line.time.as_deref().unwrap_or("00:00.00");
                    let orig = line.original.as_deref().unwrap_or("");
                    let romaji = line.romaji.as_deref().unwrap_or("");

                    div()
                        .id(ElementId::NamedInteger("lrc-line".into(), idx as u64))
                        .flex()
                        .items_start()
                        .gap_3()
                        .py_1()
                        .px_2()
                        .rounded_md()
                        .hover(|s| s.bg(theme.secondary_hover))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .w(px(55.))
                                .flex_shrink_0()
                                .child(time.to_string()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .flex_1()
                                .when(mode == LrcMode::Dual, |col| {
                                    col.child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.foreground)
                                            .child(orig.to_string()),
                                    )
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if mode == LrcMode::Dual { theme.primary } else { theme.foreground })
                                        .child(romaji.to_string()),
                                ),
                        )
                        .into_any_element()
                })
                .collect()
        } else {
            // Render from raw text lines
            let source_text = match mode {
                LrcMode::Dual => &self.lrc_studio.dual_lrc,
                LrcMode::Romaji => &self.lrc_studio.romaji_lrc,
                _ => &self.lrc_studio.original_lrc,
            };

            source_text
                .lines()
                .enumerate()
                .map(|(idx, line)| {
                    div()
                        .id(ElementId::NamedInteger("raw-line".into(), idx as u64))
                        .py_0p5()
                        .px_2()
                        .text_xs()
                        .text_color(theme.foreground)
                        .hover(|s| s.bg(theme.secondary_hover))
                        .child(line.to_string())
                        .into_any_element()
                })
                .collect()
        }
    }

    fn detect_current_song(&mut self, cx: &mut Context<Self>) {
        let track = self.playback.read(cx).track().cloned();
        if let Some(track) = track {
            let title = track.name.clone();
            let artist = track.artists.clone();
            let local_path = track
                .id
                .as_deref()
                .and_then(|id| id.strip_prefix(music::LOCAL_TRACK_PREFIX).map(Path::new))
                .map(PathBuf::from);

            if self.lrc_studio.detected_title.as_deref() != Some(&title) {
                self.lrc_studio.detected_title = Some(title.clone());
                self.lrc_studio.detected_artist = Some(artist.clone());
                self.lrc_studio.detected_local_path = local_path;

                // Auto set search input text if empty
                if self.lrc_studio.search_input.read(cx).text().is_empty() {
                    let query = format!("{title} {artist}").trim().to_string();
                    self.lrc_studio
                        .search_input
                        .update(cx, |input, cx| input.set_text(query, cx));
                }
            }
        }
    }

    fn trigger_detect_and_search(&mut self, cx: &mut Context<Self>) {
        self.detect_current_song(cx);
        if let (Some(title), Some(artist)) = (
            self.lrc_studio.detected_title.clone(),
            self.lrc_studio.detected_artist.clone(),
        ) {
            let query = format!("{title} {artist}").trim().to_string();
            self.lrc_studio
                .search_input
                .update(cx, |input, cx| input.set_text(query, cx));
            self.trigger_search(cx);
        }
    }

    fn trigger_search(&mut self, cx: &mut Context<Self>) {
        let query = self.lrc_studio.search_input.read(cx).text().trim().to_string();
        if query.is_empty() {
            return;
        }

        self.lrc_studio.is_searching = true;
        self.lrc_studio.status = None;
        cx.notify();

        let client = reqwest::Client::new();
        let url = format!("{API_SEARCH}?q={}", urlencoding(&query));

        cx.spawn(async move |this, cx| {
            let res = client
                .get(&url)
                .header(reqwest::header::USER_AGENT, "Oto/1.0")
                .send()
                .await;

            let result = match res {
                Ok(resp) => resp.json::<LrcSearchResponse>().await.ok(),
                Err(_) => None,
            };

            this.update(cx, |this, cx| {
                this.lrc_studio.is_searching = false;
                if let Some(res) = result {
                    if let Some(items) = res.data {
                        let count = items.len();
                        this.lrc_studio.results = items;
                        this.lrc_studio.status = Some((format!("Found {count} track(s)"), false));
                    } else {
                        this.lrc_studio.results.clear();
                        this.lrc_studio.status = Some(("No tracks found".into(), true));
                    }
                } else {
                    this.lrc_studio.status = Some(("Failed to connect to lyrics server".into(), true));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn select_track(&mut self, item: LrcTrackItem, cx: &mut Context<Self>) {
        self.lrc_studio.selected_track = Some(item.clone());
        let lyrics = item.synced_lyrics.or(item.plain_lyrics).unwrap_or_default();

        self.lrc_studio.original_lrc = lyrics.clone();
        self.lrc_studio.romaji_lrc = String::new();
        self.lrc_studio.dual_lrc = lyrics.clone();
        self.lrc_studio.lines.clear();
        self.lrc_studio.mode = LrcMode::Dual;

        self.lrc_studio
            .edit_input
            .update(cx, |input, cx| input.set_text(lyrics, cx));

        let has_synced = self.lrc_studio.selected_track.as_ref().and_then(|t| t.synced_lyrics.as_ref()).is_some();
        if !has_synced {
            self.lrc_studio.status = Some(("Track has plain (unsynced) lyrics".into(), false));
        } else {
            self.lrc_studio.status = Some(("Loaded synchronized lyrics".into(), false));
        }
        cx.notify();
    }

    fn set_lrc_mode(&mut self, mode: LrcMode, cx: &mut Context<Self>) {
        if self.lrc_studio.mode == LrcMode::Edit {
            let edited = self.lrc_studio.edit_input.read(cx).text().to_string();
            self.lrc_studio.dual_lrc = edited;
        }

        self.lrc_studio.mode = mode;
        let content_to_show = match mode {
            LrcMode::Dual => &self.lrc_studio.dual_lrc,
            LrcMode::Romaji => &self.lrc_studio.romaji_lrc,
            LrcMode::Original => &self.lrc_studio.original_lrc,
            LrcMode::Edit => &self.lrc_studio.dual_lrc,
        };

        let copy = content_to_show.clone();
        self.lrc_studio
            .edit_input
            .update(cx, |input, cx| input.set_text(copy, cx));
        cx.notify();
    }

    fn trigger_convert_romaji(&mut self, cx: &mut Context<Self>) {
        let lyrics = if !self.lrc_studio.original_lrc.is_empty() {
            self.lrc_studio.original_lrc.clone()
        } else {
            self.lrc_studio.edit_input.read(cx).text().to_string()
        };

        if lyrics.is_empty() {
            self.lrc_studio.status = Some(("No lyrics to convert".into(), true));
            cx.notify();
            return;
        }

        self.lrc_studio.is_converting = true;
        self.lrc_studio.status = Some(("Converting to Romaji via Gemini...".into(), false));
        cx.notify();

        let client = reqwest::Client::new();
        let payload = RomajiRequest { lyrics };

        cx.spawn(async move |this, cx| {
            let res = client
                .post(API_ROMAJI)
                .header(reqwest::header::USER_AGENT, "Oto/1.0")
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .json(&payload)
                .send()
                .await;

            let result = match res {
                Ok(resp) => resp.json::<LrcRomajiResponse>().await.ok(),
                Err(_) => None,
            };

            this.update(cx, |this, cx| {
                this.lrc_studio.is_converting = false;
                if let Some(resp) = result {
                    if resp.success {
                        this.lrc_studio.romaji_lrc = resp.romaji_lrc.unwrap_or_default();
                        this.lrc_studio.dual_lrc = resp.dual_lrc.unwrap_or_default();
                        if let Some(lines) = resp.lines {
                            this.lrc_studio.lines = lines;
                        }
                        this.lrc_studio.mode = LrcMode::Dual;

                        let dual = this.lrc_studio.dual_lrc.clone();
                        this.lrc_studio
                            .edit_input
                            .update(cx, |input, cx| input.set_text(dual, cx));

                        if resp.is_japanese == Some(false) {
                            this.lrc_studio.status = Some(("No Japanese characters detected".into(), false));
                        } else {
                            this.lrc_studio.status = Some(("Successfully converted to Dual-Line Romaji!".into(), false));
                        }
                    } else {
                        this.lrc_studio.status = Some((resp.message.unwrap_or_else(|| "Conversion failed".into()), true));
                    }
                } else {
                    this.lrc_studio.status = Some(("API request failed or quota exceeded".into(), true));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn current_export_lyrics(&self, cx: &Context<Self>) -> String {
        match self.lrc_studio.mode {
            LrcMode::Dual => {
                if !self.lrc_studio.dual_lrc.is_empty() {
                    self.lrc_studio.dual_lrc.clone()
                } else {
                    self.lrc_studio.original_lrc.clone()
                }
            }
            LrcMode::Romaji => {
                if !self.lrc_studio.romaji_lrc.is_empty() {
                    self.lrc_studio.romaji_lrc.clone()
                } else {
                    self.lrc_studio.original_lrc.clone()
                }
            }
            LrcMode::Original => self.lrc_studio.original_lrc.clone(),
            LrcMode::Edit => self.lrc_studio.edit_input.read(cx).text().to_string(),
        }
    }

    fn save_beside_song(&mut self, cx: &mut Context<Self>) {
        let Some(song_path) = self.lrc_studio.detected_local_path.clone() else {
            self.lrc_studio.status = Some(("No local audio file path found for current track".into(), true));
            cx.notify();
            return;
        };

        let lrc_path = song_path.with_extension("lrc");
        let content = self.current_export_lyrics(cx);

        if content.trim().is_empty() {
            self.lrc_studio.status = Some(("Lyrics content is empty".into(), true));
            cx.notify();
            return;
        }

        match fs::write(&lrc_path, content) {
            Ok(_) => {
                let filename = lrc_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                self.lrc_studio.status = Some((format!("Saved successfully to {filename} beside audio file!"), false));
            }
            Err(err) => {
                self.lrc_studio.status = Some((format!("Failed to save file: {err}"), true));
            }
        }
        cx.notify();
    }

    fn download_lrc_file(&mut self, cx: &mut Context<Self>) {
        let content = self.current_export_lyrics(cx);
        if content.trim().is_empty() {
            self.lrc_studio.status = Some(("Lyrics content is empty".into(), true));
            cx.notify();
            return;
        }

        let filename = match (
            self.lrc_studio.detected_title.as_deref(),
            self.lrc_studio.detected_artist.as_deref(),
        ) {
            (Some(title), Some(artist)) => format!("{artist} - {title}.lrc"),
            (Some(title), None) => format!("{title}.lrc"),
            _ => "lyrics.lrc".to_string(),
        };

        let target_dir = dirs::download_dir()
            .or_else(dirs::audio_dir)
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        let target_file = target_dir.join(&filename);
        match fs::write(&target_file, content) {
            Ok(_) => {
                self.lrc_studio.status = Some((format!("Downloaded to {}", target_file.display()), false));
            }
            Err(err) => {
                self.lrc_studio.status = Some((format!("Download failed: {err}"), true));
            }
        }
        cx.notify();
    }
}

fn urlencoding(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}
