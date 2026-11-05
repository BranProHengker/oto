use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    App, Context, Entity, FocusHandle, FontWeight, Global, Render, SharedString, Task, Window, div,
    px,
};
use i18n::t;
use music::import::{ExternalPlaylist, ExternalTrack};
use music::{MusicApi, Track};
use state::{Io, Outcome, Shelf, Sonora, Toast, Toasts};
use ui::{ActiveTheme as _, Button, Modal};
use ui::{Dismiss, FORM_CONTEXT, Input, Submit};

#[derive(Clone, Debug)]
pub struct MatchedItem {
    pub title: String,
    pub artist: String,
    pub matched_track: Option<Track>,
    pub is_local: bool,
}

enum Step {
    Input,
    Loading {
        status: SharedString,
        progress: Option<f32>,
    },
    Review {
        items: Vec<MatchedItem>,
    },
    Error(SharedString),
}

pub(crate) struct PlaylistImporter {
    open: bool,
    step: Step,
    url_input: Entity<Input>,
    name_input: Entity<Input>,
    task: Option<Task<()>>,
    background_toast: Option<usize>,
    focus: FocusHandle,
    restore: Option<FocusHandle>,
}

struct Installed(Entity<PlaylistImporter>);

impl Global for Installed {}

impl PlaylistImporter {
    pub fn entity(cx: &mut App) -> Entity<Self> {
        if cx.try_global::<Installed>().is_none() {
            let importer = cx.new(|cx| Self {
                open: false,
                step: Step::Input,
                url_input: cx.new(|cx| Input::new("playlist-url-placeholder", cx)),
                name_input: cx.new(|cx| Input::new("playlist-name-placeholder", cx)),
                task: None,
                background_toast: None,
                focus: cx.focus_handle(),
                restore: None,
            });
            cx.set_global(Installed(importer));
        }
        cx.global::<Installed>().0.clone()
    }

    pub fn open(window: &mut Window, cx: &mut App) {
        let importer = Self::entity(cx);
        importer.update(cx, |this, cx| this.show(window, cx));
    }

    fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.restore = window.focused(cx);
        match &self.step {
            Step::Input => {
                self.url_input.update(cx, |input, cx| {
                    input.set_text(String::new(), cx);
                    input.focus(window, cx);
                });
            }
            Step::Loading { .. } | Step::Review { .. } | Step::Error(_) => {}
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        if matches!(self.step, Step::Input | Step::Error(_)) {
            self.step = Step::Input;
            self.task = None;
        }
        if let Some(focus) = self.restore.take() {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn cancel_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.abort(cx);
        self.open = false;
        if let Some(focus) = self.restore.take() {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn abort(&mut self, cx: &mut Context<Self>) {
        self.task = None;
        if let Some(toast_id) = self.background_toast.take() {
            Toasts::close(toast_id, cx);
        }
        self.step = Step::Input;
        cx.notify();
    }

    fn minimize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        if let Some(focus) = self.restore.take() {
            window.focus(&focus, cx);
        }
        if self.background_toast.is_none() {
            let importer = cx.entity().clone();
            let progress = match &self.step {
                Step::Loading { progress, .. } => *progress,
                _ => None,
            };
            let mut toast = Toast::new(Outcome::Loading, "toast-playlist-importing")
                .lasting()
                .cancel("common-cancel", move |cx| {
                    importer.update(cx, |this, cx| this.abort(cx));
                });
            if let Some(p) = progress {
                toast = toast.progress(p);
            }
            let toast_id = Toasts::post(toast, cx);
            self.background_toast = Some(toast_id);
        }
        cx.notify();
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step = Step::Input;
        self.task = None;
        self.url_input.update(cx, |input, cx| {
            input.set_text(String::new(), cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn start_fetch(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).text().trim().to_string();
        if url.is_empty() {
            return;
        }

        self.step = Step::Loading {
            status: SharedString::from("Fetching playlist metadata..."),
            progress: None,
        };
        cx.notify();

        let session = Sonora::global(cx).session.clone();
        let local_client = session.read(cx).client_of(Shelf::Local);
        let streaming_client = session.read(cx).client_of(Shelf::Streaming);
        let io = Io::global(cx);

        self.task = Some(cx.spawn(async move |this, cx| {
            let result = io
                .spawn(async move { music::import::fetch_playlist(&url).await })
                .await;

            match result {
                Ok(Ok(playlist)) => {
                    this.update(cx, |this, cx| {
                        this.match_tracks(playlist, local_client, streaming_client, cx);
                    })
                    .ok();
                }
                Ok(Err(err)) => {
                    this.update(cx, |this, cx| {
                        let err_str = format!("{err:#}");
                        if let Some(toast_id) = this.background_toast.take() {
                            Toasts::settle(
                                toast_id,
                                Toast::new(Outcome::Failed, "toast-playlist-import-failed")
                                    .detail(err_str.clone()),
                                cx,
                            );
                        }
                        this.step = Step::Error(SharedString::from(err_str));
                        this.task = None;
                        cx.notify();
                    })
                    .ok();
                }
                Err(join_err) => {
                    this.update(cx, |this, cx| {
                        let err_str = format!("{join_err:#}");
                        if let Some(toast_id) = this.background_toast.take() {
                            Toasts::settle(
                                toast_id,
                                Toast::new(Outcome::Failed, "toast-playlist-import-failed")
                                    .detail(err_str.clone()),
                                cx,
                            );
                        }
                        this.step = Step::Error(SharedString::from(err_str));
                        this.task = None;
                        cx.notify();
                    })
                    .ok();
                }
            }
        }));
    }

    fn match_tracks(
        &mut self,
        playlist: ExternalPlaylist,
        local: Option<Arc<dyn MusicApi>>,
        streaming: Option<Arc<dyn MusicApi>>,
        cx: &mut Context<Self>,
    ) {
        let tracks = playlist.tracks;
        let total = tracks.len();
        let initial_name = playlist.title;
        let io = Io::global(cx);

        self.step = Step::Loading {
            status: SharedString::from(format!("Matching tracks (0/{total})...")),
            progress: Some(0.0),
        };

        if let Some(toast_id) = self.background_toast {
            let importer = cx.entity().clone();
            let toast = Toast::new(Outcome::Loading, "toast-playlist-importing")
                .name(initial_name.clone())
                .progress(0.0)
                .lasting()
                .cancel("common-cancel", move |cx| {
                    importer.update(cx, |this, cx| this.abort(cx));
                });
            Toasts::settle(toast_id, toast, cx);
        }
        cx.notify();

        self.task = Some(cx.spawn(async move |this, cx| {
            let (tx, mut rx) = tokio::sync::mpsc::channel(32);
            let semaphore = Arc::new(tokio::sync::Semaphore::new(6));

            let _dispatcher = io.spawn({
                let tracks = tracks.clone();
                let local = local.clone();
                let streaming = streaming.clone();
                async move {
                    for (index, item) in tracks.into_iter().enumerate() {
                        let sem = semaphore.clone();
                        let tx = tx.clone();
                        let local = local.clone();
                        let streaming = streaming.clone();
                        tokio::spawn(async move {
                            let _permit = sem.acquire().await;
                            let match_result = resolve_match(item, local, streaming).await;
                            let _ = tx.send((index, match_result)).await;
                        });
                    }
                }
            });

            let mut matched_items: Vec<Option<MatchedItem>> = (0..total).map(|_| None).collect();
            let mut completed = 0;

            while let Some((index, item)) = rx.recv().await {
                if index < matched_items.len() {
                    matched_items[index] = Some(item);
                }
                completed += 1;
                let progress_ratio = if total > 0 {
                    completed as f32 / total as f32
                } else {
                    1.0
                };

                this.update(cx, |this, cx| {
                    this.step = Step::Loading {
                        status: SharedString::from(format!(
                            "Matching tracks ({completed}/{total})..."
                        )),
                        progress: Some(progress_ratio),
                    };
                    if let Some(toast_id) = this.background_toast {
                        Toasts::progress(toast_id, progress_ratio, cx);
                    }
                    cx.notify();
                })
                .ok();
            }

            let final_items: Vec<MatchedItem> = matched_items.into_iter().flatten().collect();

            this.update(cx, |this, cx| {
                if this.open {
                    this.name_input.update(cx, |input, cx| {
                        input.set_text(initial_name.clone(), cx);
                    });
                    this.step = Step::Review { items: final_items };
                    cx.notify();
                } else {
                    this.auto_save_background(initial_name, final_items, cx);
                }
            })
            .ok();
        }));
    }

    fn auto_save_background(
        &mut self,
        name: String,
        items: Vec<MatchedItem>,
        cx: &mut Context<Self>,
    ) {
        if let Some(toast_id) = self.background_toast.take() {
            Toasts::close(toast_id, cx);
        }

        let tracks_to_save: Vec<Track> = items
            .iter()
            .filter_map(|item| item.matched_track.clone())
            .collect();

        if tracks_to_save.is_empty() {
            Toasts::with_detail(
                Outcome::Failed,
                "toast-playlist-import-failed",
                Some(SharedString::from(name)),
                "No tracks could be matched",
                cx,
            );
            self.step = Step::Input;
            self.task = None;
            return;
        }

        let library = Sonora::global(cx).library.clone();
        library.update(cx, |lib, cx| {
            lib.create_hybrid_playlist(name, tracks_to_save, cx);
        });

        self.step = Step::Input;
        self.task = None;
    }

    fn apply_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Step::Review { items, .. } = &self.step else {
            return;
        };

        let name = self.name_input.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }

        let tracks_to_save: Vec<Track> = items
            .iter()
            .filter_map(|item| item.matched_track.clone())
            .collect();

        if tracks_to_save.is_empty() {
            self.step = Step::Error(SharedString::from("No tracks could be matched to save."));
            cx.notify();
            return;
        }

        let library = Sonora::global(cx).library.clone();
        library.update(cx, |lib, cx| {
            lib.create_hybrid_playlist(name, tracks_to_save, cx);
        });

        self.close(window, cx);
    }
}

async fn resolve_match(
    item: ExternalTrack,
    local: Option<Arc<dyn MusicApi>>,
    streaming: Option<Arc<dyn MusicApi>>,
) -> MatchedItem {
    let clean_title = clean_query(&item.title);
    let clean_artist = clean_query(&item.artist);

    // 1. Try local match first (highest audio quality)
    if let Some(local_api) = &local
        && let Ok(results) = local_api.search(&clean_title).await
    {
        for track in results {
            let track_title = clean_query(&track.name);
            let track_artist = clean_query(&track.artists);
            if (track_title.contains(&clean_title) || clean_title.contains(&track_title))
                && (track_artist.contains(&clean_artist) || clean_artist.contains(&track_artist))
            {
                return MatchedItem {
                    title: item.title,
                    artist: item.artist,
                    matched_track: Some(track),
                    is_local: true,
                };
            }
        }
    }

    // 2. Try streaming provider match
    if let Some(stream_api) = &streaming {
        let query = format!("{} {}", item.artist, item.title);
        if let Ok(results) = stream_api.search(&query).await
            && let Some(best) = find_best_streaming_match(&item, results)
        {
            return MatchedItem {
                title: item.title,
                artist: item.artist,
                matched_track: Some(best),
                is_local: false,
            };
        }
    }

    MatchedItem {
        title: item.title,
        artist: item.artist,
        matched_track: None,
        is_local: false,
    }
}

fn find_best_streaming_match(item: &ExternalTrack, candidates: Vec<Track>) -> Option<Track> {
    let target_title = clean_query(&item.title);
    let target_artist = clean_query(&item.artist);

    candidates.into_iter().find(|track| {
        let candidate_title = clean_query(&track.name);
        let candidate_artist = clean_query(&track.artists);
        let title_match =
            candidate_title.contains(&target_title) || target_title.contains(&candidate_title);
        let artist_match =
            candidate_artist.contains(&target_artist) || target_artist.contains(&candidate_artist);

        if let Some(dur) = item.duration {
            let delta = dur.abs_diff(track.duration);
            title_match && (artist_match || delta < Duration::from_secs(5))
        } else {
            title_match && artist_match
        }
    })
}

fn clean_query(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl Render for PlaylistImporter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }

        let theme = *cx.theme();
        div()
            .key_context(FORM_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| {
                cx.stop_propagation();
                match &this.step {
                    Step::Loading { .. } => this.minimize(window, cx),
                    _ => this.close(window, cx),
                }
            }))
            .on_action(cx.listener(|this, _: &Submit, window, cx| {
                cx.stop_propagation();
                match &this.step {
                    Step::Input => this.start_fetch(window, cx),
                    Step::Review { .. } => this.apply_import(window, cx),
                    _ => {}
                }
            }))
            .child(
                Modal::new("playlist-importer", "Import Playlist")
                    .w(theme.metrics.cover * 3.6)
                    .map(|modal| match &self.step {
                        Step::Input => modal
                            .detail("Paste a public playlist URL from Spotify, YouTube Music, Apple Music, or Deezer.")
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(self.url_input.clone())
                            ),
                        Step::Loading { status, progress } => modal
                            .detail(status.clone())
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .py_4()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Matching tracks from your local library and streaming accounts concurrently..."),
                                    )
                                    .when_some(*progress, |parent, p| {
                                        let percent = (p * 100.0) as u32;
                                        parent.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .h(px(6.))
                                                        .rounded_full()
                                                        .bg(theme.border)
                                                        .child(
                                                            div()
                                                                .h_full()
                                                                .w(gpui::DefiniteLength::Fraction(p))
                                                                .rounded_full()
                                                                .bg(theme.primary),
                                                        )
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .justify_between()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child(format!("{percent}%"))
                                                        .child("Click “Run in background” to minimize"),
                                                )
                                        )
                                    })
                            ),
                        Step::Review { items, .. } => {
                            let matched_count = items.iter().filter(|i| i.matched_track.is_some()).count();
                            let local_count = items.iter().filter(|i| i.is_local).count();
                            let streaming_count = matched_count.saturating_sub(local_count);
                            let total = items.len();

                            let summary = format!(
                                "{matched_count}/{total} matched ({local_count} Local Lossless, {streaming_count} Streaming)"
                            );

                            modal
                                .detail(SharedString::from(summary))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(div().text_size(theme.text(ui::Text::Small)).text_color(theme.muted_foreground).child("Playlist Name:"))
                                                .child(self.name_input.clone())
                                        )
                                        .child(
                                            div()
                                                .id("import-items-scroll")
                                                .flex()
                                                .flex_col()
                                                .max_h(px(220.))
                                                .overflow_y_scroll()
                                                .gap_1()
                                                .p_1()
                                                .bg(theme.background)
                                                .rounded(theme.radius)
                                                .children(items.iter().map(|item| {
                                                    let (tag, color) = match (item.matched_track.is_some(), item.is_local) {
                                                        (true, true) => ("Local FLAC", theme.primary),
                                                        (true, false) => ("Streaming", theme.foreground),
                                                        (false, _) => ("Not Found", theme.muted_foreground),
                                                    };

                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .py_1()
                                                        .px_2()
                                                        .text_size(theme.text(ui::Text::Tiny))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .child(div().font_weight(FontWeight::MEDIUM).child(item.title.clone()))
                                                                .child(div().text_color(theme.muted_foreground).child(item.artist.clone()))
                                                        )
                                                        .child(
                                                            div()
                                                                .text_color(color)
                                                                .font_weight(FontWeight::BOLD)
                                                                .child(tag)
                                                        )
                                                }))
                                        )
                                )
                        }
                        Step::Error(err) => modal
                            .detail("Import failed")
                            .child(
                                div()
                                    .flex()
                                    .py_4()
                                    .text_color(theme.danger)
                                    .child(err.clone())
                            ),
                    })
                    .action(
                        Button::new("cancel-import")
                            .ghost()
                            .label(t!("common-cancel"))
                            .on_click(cx.listener(|this, _, window, cx| this.cancel_import(window, cx))),
                    )
                    .action(
                        match &self.step {
                            Step::Input => Button::new("fetch-import")
                                .primary()
                                .label(t!("common-confirm"))
                                .on_click(cx.listener(|this, _, window, cx| this.start_fetch(window, cx))),
                            Step::Loading { .. } => Button::new("minimize-import")
                                .primary()
                                .label(t!("common-background"))
                                .on_click(cx.listener(|this, _, window, cx| this.minimize(window, cx))),
                            Step::Review { .. } => Button::new("save-import")
                                .primary()
                                .label("Save Playlist")
                                .on_click(cx.listener(|this, _, window, cx| this.apply_import(window, cx))),
                            Step::Error(_) => Button::new("retry-import")
                                .primary()
                                .label("Try Again")
                                .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
                        }
                    )
                    .on_dismiss(cx.listener(|this, _, window, cx| {
                        match &this.step {
                            Step::Loading { .. } => this.minimize(window, cx),
                            _ => this.close(window, cx),
                        }
                    })),
            )
            .into_any_element()
    }
}
