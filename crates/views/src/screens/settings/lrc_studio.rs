use std::fs;
use std::path::{Path, PathBuf};

use gpui::prelude::*;
use gpui::{
    AnyElement, Context, ElementId, Entity, FontWeight, Window, div, px, rgb, svg,
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

#[derive(Clone, Debug, PartialEq, Default)]
pub(super) struct StudioLine {
    pub time: String,
    pub original: String,
    pub romaji: String,
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
    pub line_time_input: Entity<Input>,
    pub line_text_input: Entity<Input>,
    pub selected_line_idx: Option<usize>,
    pub lines: Vec<StudioLine>,
    pub results: Vec<LrcTrackItem>,
    pub selected_track: Option<LrcTrackItem>,
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
        let line_time_input = cx.new(|cx| {
            Input::new("lrc-studio-time-placeholder", cx).compact()
        });
        let line_text_input = cx.new(|cx| {
            Input::new("lrc-studio-edit-placeholder", cx).compact()
        });

        Self {
            search_input,
            line_time_input,
            line_text_input,
            selected_line_idx: None,
            lines: Vec::new(),
            results: Vec::new(),
            selected_track: None,
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
        let line_time_input = self.lrc_studio.line_time_input.clone();
        let line_text_input = self.lrc_studio.line_text_input.clone();
        let is_searching = self.lrc_studio.is_searching;
        let is_converting = self.lrc_studio.is_converting;
        let mode = self.lrc_studio.mode;
        let status = self.lrc_studio.status.clone();
        let has_local_path = self.lrc_studio.detected_local_path.is_some();
        let lines_count = self.lrc_studio.lines.len();
        let selected_idx = self.lrc_studio.selected_line_idx;

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

        let selected_track_name = self.lrc_studio.selected_track.as_ref().and_then(|t| t.track_name.clone());

        div()
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .child(
                // 1. Now Playing Detection Card (Zero Emojis, Native Themed SVGs)
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
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
                                        svg()
                                            .path(icons::path("icons/disc-3.svg"))
                                            .size(px(16.))
                                            .flex_shrink_0()
                                            .text_color(theme.foreground),
                                    )
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
                                    .icon("icons/refresh-cw.svg")
                                    .small()
                                    .ghost()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.trigger_detect_and_search(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                svg()
                                    .path(icons::path("icons/music.svg"))
                                    .size(px(14.))
                                    .flex_shrink_0()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.foreground)
                                    .child(detected_display),
                            ),
                    )
                    .when_some(local_path_display, |card, path| {
                        let lrc_path = format!("{}.lrc", path.trim_end_matches(".mp3").trim_end_matches(".flac").trim_end_matches(".m4a").trim_end_matches(".wav").trim_end_matches(".ogg"));
                        card.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    svg()
                                        .path(icons::path("icons/folder-plus.svg"))
                                        .size(px(14.))
                                        .flex_shrink_0()
                                        .text_color(theme.primary),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.primary)
                                        .child(format!("Target: {}", lrc_path)),
                                ),
                        )
                    }),
            )
            .child(
                // 2. Search Lyrics Card
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded_xl()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .child(div().flex_1().child(search_input))
                            .child(
                                Button::new("lrc-search-btn")
                                    .label(if is_searching { "Searching..." } else { "Search LRCLIB" })
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
                                .pt_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.muted_foreground)
                                        .child(format!("Search Results ({} tracks found) - Click any song to load into Workspace:", results.len())),
                                )
                                .child(
                                    div()
                                        .id("lrc-search-results-list")
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .max_h(px(200.))
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
                                                .bg(if is_selected { theme.selection } else { theme.background })
                                                .border_1()
                                                .border_color(if is_selected { theme.primary } else { theme.border })
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
                    }),
            )
            .child(
                // 3. Lyrics Studio Workspace Card (ALWAYS VISIBLE!)
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
                        // Header Title + Loaded Song Badge + Line Count
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        svg()
                                            .path(icons::path("icons/list-music.svg"))
                                            .size(px(16.))
                                            .flex_shrink_0()
                                            .text_color(theme.foreground),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child("Lyrics Workspace"),
                                    )
                                    .when_some(selected_track_name, |grp, name| {
                                        grp.child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_full()
                                                .bg(theme.background)
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(format!("Loaded: {name}")),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{lines_count} line(s)")),
                            ),
                    )
                    .child(
                        // Action Buttons Bar with Horizontal Scroll ("geser kanan kiri")
                        div()
                            .id("workspace-actions-scroll")
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .overflow_x_scroll()
                            .py_0p5()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .child(
                                        Button::new("btn-generate-romaji")
                                            .label(if is_converting { "Generating Romaji..." } else { "Generate Dual Romaji" })
                                            .icon("icons/arrow-right-left.svg")
                                            .small()
                                            .selected(!is_converting)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.trigger_convert_romaji(cx);
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .child(
                                        Button::new("btn-save-beside")
                                            .label(if has_local_path {
                                                t!("lrc-studio-save-local")
                                            } else {
                                                t!("lrc-studio-download")
                                            })
                                            .icon("icons/download.svg")
                                            .small()
                                            .ghost()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.save_beside_song(cx);
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .child(
                                        Button::new("btn-copy")
                                            .label("Copy LRC")
                                            .icon("icons/copy.svg")
                                            .small()
                                            .ghost()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.copy_lyrics_to_clipboard(cx);
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .child(
                                        Button::new("btn-paste")
                                            .label("Paste from Clipboard")
                                            .icon("icons/clipboard-paste.svg")
                                            .small()
                                            .ghost()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.paste_lyrics_from_clipboard(cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        // Mode Switcher Tabs with Horizontal Scroll ("geser kanan kiri")
                        div()
                            .id("workspace-modes-scroll")
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .overflow_x_scroll()
                            .py_0p5()
                            .child(
                                TabBar::new("lrc-modes")
                                    .items([
                                        Button::new("mode-dual")
                                            .label("Dual-Line Romaji")
                                            .icon("icons/arrow-right-left.svg")
                                            .small()
                                            .ghost()
                                            .selected(mode == LrcMode::Dual)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_lrc_mode(LrcMode::Dual, cx);
                                            })),
                                        Button::new("mode-romaji")
                                            .label("Romaji Only")
                                            .icon("icons/music-2.svg")
                                            .small()
                                            .ghost()
                                            .selected(mode == LrcMode::Romaji)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_lrc_mode(LrcMode::Romaji, cx);
                                            })),
                                        Button::new("mode-original")
                                            .label("Original")
                                            .icon("icons/music.svg")
                                            .small()
                                            .ghost()
                                            .selected(mode == LrcMode::Original)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_lrc_mode(LrcMode::Original, cx);
                                            })),
                                        Button::new("mode-edit")
                                            .label("Edit Lines")
                                            .icon("icons/pencil.svg")
                                            .small()
                                            .ghost()
                                            .selected(mode == LrcMode::Edit)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_lrc_mode(LrcMode::Edit, cx);
                                            })),
                                    ]),
                            ),
                    )
                    .child(
                        // Mode Content Panel
                        if self.lrc_studio.lines.is_empty() {
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .p_8()
                                .rounded_lg()
                                .bg(theme.background)
                                .border_1()
                                .border_color(theme.border)
                                .child(
                                    svg()
                                        .path(icons::path("icons/list-music.svg"))
                                        .size(px(32.))
                                        .text_color(theme.muted_foreground),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child("No Lyrics in Workspace"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Search LRCLIB above, click 'Detect Current Song', or click 'Paste from Clipboard' below."),
                                )
                                .child(
                                    Button::new("btn-empty-paste")
                                        .label("Paste from Clipboard")
                                        .icon("icons/clipboard-paste.svg")
                                        .small()
                                        .ghost()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.paste_lyrics_from_clipboard(cx);
                                        })),
                                )
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .when(mode == LrcMode::Edit, |panel| {
                                    // Crash-Safe Single-Line Editor Bar for Selected Line
                                    let current_idx_display = selected_idx.map(|i| i + 1).unwrap_or(1);
                                    let total_lines = self.lrc_studio.lines.len();

                                    panel.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .p_3()
                                            .rounded_lg()
                                            .bg(theme.background)
                                            .border_1()
                                            .border_color(theme.primary)
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .w_full()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(theme.primary)
                                                            .child(format!("Line Editor (Line {current_idx_display} of {total_lines}) - Click any line below to edit:")),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap_1()
                                                            .child(
                                                                Button::new("btn-prev-line")
                                                                    .icon("icons/chevron-left.svg")
                                                                    .small()
                                                                    .ghost()
                                                                    .disabled(selected_idx.unwrap_or(0) == 0)
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.prev_line(cx);
                                                                    })),
                                                            )
                                                            .child(
                                                                Button::new("btn-next-line")
                                                                    .icon("icons/chevron-right.svg")
                                                                    .small()
                                                                    .ghost()
                                                                    .disabled(selected_idx.unwrap_or(0) + 1 >= total_lines)
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.next_line(cx);
                                                                    })),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .w_full()
                                                    .child(
                                                        div()
                                                            .w(px(90.))
                                                            .flex_shrink_0()
                                                            .child(line_time_input),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .child(line_text_input),
                                                    )
                                                    .child(
                                                        Button::new("btn-save-line")
                                                            .label("Save Line")
                                                            .icon("icons/check.svg")
                                                            .small()
                                                            .selected(true)
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.save_current_line(cx);
                                                            })),
                                                    )
                                                    .child(
                                                        Button::new("btn-add-line")
                                                            .label("Add Line")
                                                            .icon("icons/plus.svg")
                                                            .small()
                                                            .ghost()
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.insert_line_after(cx);
                                                            })),
                                                    )
                                                    .child(
                                                        Button::new("btn-del-line")
                                                            .icon("icons/trash-2.svg")
                                                            .small()
                                                            .ghost()
                                                            .destructive()
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.delete_selected_line(cx);
                                                            })),
                                                    ),
                                            ),
                                    )
                                })
                                .child(
                                    // Scrollable Lyrics View
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
                                        .children(self.render_lines(&theme, mode, selected_idx, cx)),
                                )
                                .into_any_element()
                        },
                    ),
            )
            .into_any_element()
    }

    fn render_lines(
        &self,
        theme: &ui::Theme,
        mode: LrcMode,
        selected_idx: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.lrc_studio
            .lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let is_selected = selected_idx == Some(idx);
                let time = if line.time.is_empty() { "00:00.00" } else { &line.time };
                let has_romaji = !line.romaji.is_empty();

                div()
                    .id(ElementId::NamedInteger("lrc-line".into(), idx as u64))
                    .flex()
                    .items_start()
                    .gap_3()
                    .py_1()
                    .px_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_selected && mode == LrcMode::Edit {
                        theme.selection
                    } else {
                        theme.background
                    })
                    .border_1()
                    .border_color(if is_selected && mode == LrcMode::Edit {
                        theme.primary
                    } else {
                        theme.border
                    })
                    .hover(|s| s.bg(theme.secondary_hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_line_for_edit(idx, cx);
                    }))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .w(px(65.))
                            .flex_shrink_0()
                            .child(format!("[{time}]")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .flex_1()
                            .when(mode != LrcMode::Romaji, |col| {
                                col.child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .child(line.original.clone()),
                                )
                            })
                            .when(
                                (mode == LrcMode::Dual || mode == LrcMode::Romaji) && has_romaji,
                                |col| {
                                    col.child(
                                        div()
                                            .text_xs()
                                            .text_color(if mode == LrcMode::Dual {
                                                theme.primary
                                            } else {
                                                theme.foreground
                                            })
                                            .child(line.romaji.clone()),
                                    )
                                },
                            ),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn select_line_for_edit(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.lrc_studio.lines.len() {
            return;
        }

        self.lrc_studio.selected_line_idx = Some(idx);
        let line = &self.lrc_studio.lines[idx];
        let time = line.time.clone();
        let text = match self.lrc_studio.mode {
            LrcMode::Romaji if !line.romaji.is_empty() => line.romaji.clone(),
            _ => line.original.clone(),
        };

        self.lrc_studio
            .line_time_input
            .update(cx, |input, cx| input.set_text(time, cx));
        self.lrc_studio
            .line_text_input
            .update(cx, |input, cx| input.set_text(text, cx));
        cx.notify();
    }

    fn save_current_line(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.lrc_studio.selected_line_idx else {
            return;
        };
        if idx >= self.lrc_studio.lines.len() {
            return;
        }

        let new_time = self.lrc_studio.line_time_input.read(cx).text().trim().to_string();
        let new_text = self.lrc_studio.line_text_input.read(cx).text().trim().to_string();

        let line = &mut self.lrc_studio.lines[idx];
        if !new_time.is_empty() {
            line.time = new_time;
        }

        if self.lrc_studio.mode == LrcMode::Romaji && !line.romaji.is_empty() {
            line.romaji = new_text;
        } else {
            line.original = new_text;
        }

        self.lrc_studio.status = Some((format!("Updated Line #{}", idx + 1), false));
        cx.notify();
    }

    fn insert_line_after(&mut self, cx: &mut Context<Self>) {
        let idx = self.lrc_studio.selected_line_idx.unwrap_or(self.lrc_studio.lines.len());
        let insert_at = (idx + 1).min(self.lrc_studio.lines.len());

        let prev_time = if idx < self.lrc_studio.lines.len() {
            self.lrc_studio.lines[idx].time.clone()
        } else {
            "00:00.00".to_string()
        };

        self.lrc_studio.lines.insert(
            insert_at,
            StudioLine {
                time: prev_time,
                original: "New lyric line".to_string(),
                romaji: String::new(),
            },
        );

        self.select_line_for_edit(insert_at, cx);
        self.lrc_studio.status = Some((format!("Inserted Line #{}", insert_at + 1), false));
        cx.notify();
    }

    fn delete_selected_line(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.lrc_studio.selected_line_idx else {
            return;
        };
        if idx >= self.lrc_studio.lines.len() {
            return;
        }

        self.lrc_studio.lines.remove(idx);
        let next_idx = if self.lrc_studio.lines.is_empty() {
            None
        } else {
            Some(idx.min(self.lrc_studio.lines.len() - 1))
        };

        if let Some(i) = next_idx {
            self.select_line_for_edit(i, cx);
        } else {
            self.lrc_studio.selected_line_idx = None;
            self.lrc_studio
                .line_time_input
                .update(cx, |input, cx| input.set_text("", cx));
            self.lrc_studio
                .line_text_input
                .update(cx, |input, cx| input.set_text("", cx));
        }

        self.lrc_studio.status = Some(("Line deleted".into(), false));
        cx.notify();
    }

    fn prev_line(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.lrc_studio.selected_line_idx else {
            return;
        };
        if idx > 0 {
            self.select_line_for_edit(idx - 1, cx);
        }
    }

    fn next_line(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.lrc_studio.selected_line_idx else {
            return;
        };
        if idx + 1 < self.lrc_studio.lines.len() {
            self.select_line_for_edit(idx + 1, cx);
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

            let is_new = self.lrc_studio.detected_title.as_deref() != Some(&title);
            if is_new {
                self.lrc_studio.detected_title = Some(title.clone());
                self.lrc_studio.detected_artist = Some(artist.clone());
                self.lrc_studio.detected_local_path = local_path.clone();

                // If local path has an existing .lrc file, auto-load it!
                if let Some(ref p) = local_path {
                    let mut candidate = p.clone();
                    candidate.set_extension("lrc");
                    if let Ok(content) = fs::read_to_string(&candidate) {
                        let trimmed = content.trim();
                        if !trimmed.is_empty() {
                            self.load_raw_lrc_string(trimmed, cx);
                            self.lrc_studio.status = Some(("Loaded existing .lrc from local music folder!".into(), false));
                        }
                    }
                }

                // Fill search input if empty
                if self.lrc_studio.search_input.read(cx).text().is_empty() {
                    let query = if artist.is_empty() {
                        title
                    } else {
                        format!("{title} {artist}")
                    };
                    self.lrc_studio
                        .search_input
                        .update(cx, |input, cx| input.set_text(query, cx));
                }
            }
        }
    }

    fn trigger_detect_and_search(&mut self, cx: &mut Context<Self>) {
        let track = self.playback.read(cx).track().cloned();
        if let Some(track) = track {
            let title = track.name.clone();
            let artist = track.artists.clone();
            let local_path = track
                .id
                .as_deref()
                .and_then(|id| id.strip_prefix(music::LOCAL_TRACK_PREFIX).map(Path::new))
                .map(PathBuf::from);

            self.lrc_studio.detected_title = Some(title.clone());
            self.lrc_studio.detected_artist = Some(artist.clone());
            self.lrc_studio.detected_local_path = local_path.clone();

            // Check if local file has .lrc
            if let Some(ref p) = local_path {
                let mut candidate = p.clone();
                candidate.set_extension("lrc");
                if let Ok(content) = fs::read_to_string(&candidate) {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        self.load_raw_lrc_string(trimmed, cx);
                        self.lrc_studio.status = Some(("Loaded existing .lrc from song folder!".into(), false));
                    }
                }
            }

            let query = if artist.is_empty() {
                title
            } else {
                format!("{title} {artist}")
            };
            self.lrc_studio
                .search_input
                .update(cx, |input, cx| input.set_text(query, cx));

            self.trigger_search(cx);
        } else {
            self.lrc_studio.status = Some(("No song currently playing in Oto".into(), true));
            cx.notify();
        }
    }

    fn trigger_search(&mut self, cx: &mut Context<Self>) {
        let query = self.lrc_studio.search_input.read(cx).text().trim().to_string();
        if query.is_empty() {
            return;
        }

        self.lrc_studio.is_searching = true;
        self.lrc_studio.status = Some(("Searching LRCLIB...".into(), false));
        cx.notify();

        let io = state::Io::global(cx);
        let q = query.clone();

        cx.spawn(async move |this, cx| {
            let result = io.spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()
                    .ok()?;

                let url1 = format!("{API_SEARCH}?q={}", urlencoding(&q));
                let res1 = client
                    .get(&url1)
                    .header(reqwest::header::USER_AGENT, "Oto/1.0 (https://github.com/BranProHengker/oto)")
                    .send()
                    .await
                    .ok()?;

                let parsed1 = res1.json::<LrcSearchResponse>().await.ok()?;
                if parsed1.data.as_ref().is_some_and(|items| !items.is_empty()) {
                    return Some(parsed1);
                }

                // If query had multiple words and returned 0 results, retry with first term (e.g. Song title only)
                if let Some((first, _)) = q.split_once(' ') {
                    let url2 = format!("{API_SEARCH}?q={}", urlencoding(first.trim()));
                    let res2 = client
                        .get(&url2)
                        .header(reqwest::header::USER_AGENT, "Oto/1.0 (https://github.com/BranProHengker/oto)")
                        .send()
                        .await;

                    if let Ok(resp) = res2 {
                        let parsed2 = resp.json::<LrcSearchResponse>().await.ok();
                        let has_items = parsed2.as_ref().and_then(|p| p.data.as_ref()).is_some_and(|items| !items.is_empty());
                        if has_items {
                            return parsed2;
                        }
                    }
                }

                Some(parsed1)
            }).await.ok().flatten();

            this.update(cx, |this, cx| {
                this.lrc_studio.is_searching = false;
                if let Some(res) = result {
                    if let Some(items) = res.data {
                        let count = items.len();
                        this.lrc_studio.results = items;
                        if count > 0 {
                            this.lrc_studio.status = Some((format!("Found {count} track(s) from LRCLIB"), false));
                        } else {
                            this.lrc_studio.status = Some(("No matching lyrics found on LRCLIB".into(), true));
                        }
                    } else {
                        this.lrc_studio.results.clear();
                        this.lrc_studio.status = Some(("No matching lyrics found".into(), true));
                    }
                } else {
                    this.lrc_studio.status = Some(("Failed to connect to lyrics server".into(), true));
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    fn select_track(&mut self, item: LrcTrackItem, cx: &mut Context<Self>) {
        self.lrc_studio.selected_track = Some(item.clone());
        let lyrics = item.synced_lyrics.or(item.plain_lyrics).unwrap_or_default();

        self.load_raw_lrc_string(&lyrics, cx);

        let has_synced = self.lrc_studio.selected_track.as_ref().and_then(|t| t.synced_lyrics.as_ref()).is_some();
        let track_name = item.track_name.as_deref().unwrap_or("track");
        if !has_synced {
            self.lrc_studio.status = Some((format!("Loaded plain lyrics for '{track_name}'"), false));
        } else {
            self.lrc_studio.status = Some((format!("Loaded synchronized lyrics for '{track_name}'! Click 'Generate Dual Romaji' to convert."), false));
        }
        cx.notify();
    }

    fn set_lrc_mode(&mut self, mode: LrcMode, cx: &mut Context<Self>) {
        self.lrc_studio.mode = mode;
        if mode == LrcMode::Edit && self.lrc_studio.selected_line_idx.is_none() && !self.lrc_studio.lines.is_empty() {
            self.select_line_for_edit(0, cx);
        }
        cx.notify();
    }

    fn load_raw_lrc_string(&mut self, text: &str, cx: &mut Context<Self>) {
        self.lrc_studio.lines = parse_lrc_lines(text);
        if !self.lrc_studio.lines.is_empty() {
            self.select_line_for_edit(0, cx);
        } else {
            self.lrc_studio.selected_line_idx = None;
        }
    }

    fn trigger_convert_romaji(&mut self, cx: &mut Context<Self>) {
        if self.lrc_studio.lines.is_empty() {
            self.lrc_studio.status = Some(("No lyrics to convert. Please search or paste lyrics first.".into(), true));
            cx.notify();
            return;
        }

        let lyrics = self.generate_lrc_content(LrcMode::Original);

        self.lrc_studio.is_converting = true;
        self.lrc_studio.status = Some(("Generating Dual-Line Romaji via Gemini...".into(), false));
        cx.notify();

        let io = state::Io::global(cx);
        let payload = RomajiRequest { lyrics };

        cx.spawn(async move |this, cx| {
            let result = io.spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .build()
                    .ok()?;
                let res = client
                    .post(API_ROMAJI)
                    .header(reqwest::header::USER_AGENT, "Oto/1.0 (https://github.com/BranProHengker/oto)")
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .json(&payload)
                    .send()
                    .await
                    .ok()?;
                res.json::<LrcRomajiResponse>().await.ok()
            }).await.ok().flatten();

            this.update(cx, |this, cx| {
                this.lrc_studio.is_converting = false;
                if let Some(resp) = result {
                    if resp.success {
                        if let Some(api_lines) = resp.lines {
                            this.lrc_studio.lines = api_lines
                                .into_iter()
                                .map(|l| StudioLine {
                                    time: l.time.unwrap_or_else(|| "00:00.00".into()),
                                    original: l.original.unwrap_or_default(),
                                    romaji: l.romaji.unwrap_or_default(),
                                })
                                .collect();
                        }
                        this.lrc_studio.mode = LrcMode::Dual;
                        if !this.lrc_studio.lines.is_empty() {
                            this.select_line_for_edit(0, cx);
                        }

                        if resp.is_japanese == Some(false) {
                            this.lrc_studio.status = Some(("Lyrics processed (no Japanese characters detected)".into(), false));
                        } else {
                            this.lrc_studio.status = Some(("Successfully generated Dual-Line Romaji!".into(), false));
                        }
                    } else {
                        let err_msg = resp.message.unwrap_or_else(|| "Conversion failed".into());
                        this.lrc_studio.status = Some((format!("Error: {err_msg}"), true));
                    }
                } else {
                    this.lrc_studio.status = Some(("Failed to contact Romaji server or timeout".into(), true));
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    fn generate_lrc_content(&self, mode: LrcMode) -> String {
        let mut out = String::new();
        for line in &self.lrc_studio.lines {
            match mode {
                LrcMode::Dual => {
                    if !line.original.is_empty() {
                        out.push_str(&format!("[{}] {}\n", line.time, line.original));
                    }
                    if !line.romaji.is_empty() {
                        out.push_str(&format!("[{}] {}\n", line.time, line.romaji));
                    }
                }
                LrcMode::Romaji => {
                    let text = if !line.romaji.is_empty() {
                        &line.romaji
                    } else {
                        &line.original
                    };
                    out.push_str(&format!("[{}] {}\n", line.time, text));
                }
                LrcMode::Original | LrcMode::Edit => {
                    out.push_str(&format!("[{}] {}\n", line.time, line.original));
                }
            }
        }
        out
    }

    fn save_beside_song(&mut self, cx: &mut Context<Self>) {
        if self.lrc_studio.lines.is_empty() {
            self.lrc_studio.status = Some(("Lyrics content is empty".into(), true));
            cx.notify();
            return;
        }

        let content = self.generate_lrc_content(self.lrc_studio.mode);
        let target = if let Some(ref local_path) = self.lrc_studio.detected_local_path {
            let mut p = local_path.clone();
            p.set_extension("lrc");
            p
        } else {
            let filename = match (
                self.lrc_studio.detected_title.as_deref(),
                self.lrc_studio.detected_artist.as_deref(),
            ) {
                (Some(title), Some(artist)) => format!("{artist} - {title}.lrc"),
                (Some(title), None) => format!("{title}.lrc"),
                _ => "lyrics.lrc".to_string(),
            };
            let safe_filename = filename.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
            let dest_dir = dirs::audio_dir().or_else(dirs::download_dir).unwrap_or_else(|| PathBuf::from("."));
            dest_dir.join(&safe_filename)
        };

        match fs::write(&target, &content) {
            Ok(_) => {
                let file_name = target.file_name().unwrap_or_default().to_string_lossy();
                if self.lrc_studio.detected_local_path.is_some() {
                    self.lrc_studio.status = Some((format!("Saved {file_name} successfully beside song!"), false));
                } else {
                    self.lrc_studio.status = Some((format!("Saved to {}", target.display()), false));
                }
            }
            Err(e) => {
                self.lrc_studio.status = Some((format!("Failed to save .lrc: {e}"), true));
            }
        }
        cx.notify();
    }

    fn copy_lyrics_to_clipboard(&mut self, cx: &mut Context<Self>) {
        if self.lrc_studio.lines.is_empty() {
            self.lrc_studio.status = Some(("No lyrics to copy".into(), true));
            cx.notify();
            return;
        }

        let content = self.generate_lrc_content(self.lrc_studio.mode);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(content));
        self.lrc_studio.status = Some(("Lyrics copied to clipboard!".into(), false));
        cx.notify();
    }

    fn paste_lyrics_from_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            self.lrc_studio.status = Some(("Clipboard is empty".into(), true));
            cx.notify();
            return;
        };

        if text.trim().is_empty() {
            self.lrc_studio.status = Some(("Clipboard is empty".into(), true));
            cx.notify();
            return;
        }

        self.load_raw_lrc_string(&text, cx);
        let count = self.lrc_studio.lines.len();
        self.lrc_studio.status = Some((format!("Loaded {count} lines from clipboard! Click 'Generate Dual Romaji' to convert."), false));
        cx.notify();
    }
}

fn parse_lrc_lines(content: &str) -> Vec<StudioLine> {
    let mut lines = Vec::new();
    for raw in content.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[')
            && let Some(close_idx) = trimmed.find(']')
        {
            let time = trimmed[1..close_idx].trim().to_string();
            let text = trimmed[close_idx + 1..].trim().to_string();
            lines.push(StudioLine {
                time,
                original: text,
                romaji: String::new(),
            });
            continue;
        }
        lines.push(StudioLine {
            time: "00:00.00".to_string(),
            original: trimmed.to_string(),
            romaji: String::new(),
        });
    }
    lines
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
                use std::fmt::Write as _;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}
