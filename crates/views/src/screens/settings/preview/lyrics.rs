//! A few sample lines of lyrics drawn by the lyrics sheet's own renderer the way Now playing
//! draws them, held still in the middle of a karaoke line so every lyrics setting shows the
//! moment it changes. The line being sung is in Japanese, so romanization has something to add.

use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{AnyElement, Context, Div, Pixels, Window, div};
use music::{LyricsLine, LyricsWord, Voice};
use state::RomanizationScripts;
use ui::{ActiveTheme as _, Theme};

use crate::chrome::{VerseLook, verse, verse_gap, verse_sizes};
use crate::screens::settings::{Layout, SettingsView};

/// How tall the preview stands for its width.
const ASPECT: f32 = 0.4;
/// How far down the preview the line being sung starts, as a share of its height.
const ANCHOR: f32 = 0.36;
/// The haze curve of the sheet, from a line's distance to the one being sung.
const HAZE: f32 = 0.45;
/// How far the sheet fades in from its top and bottom edges, in verse sizes, as the panel does.
const VERSE_FADE: f32 = 1.25;
/// Full animation loop duration for the live singing preview (matching Oto Android).
const LOOP_DURATION: Duration = Duration::from_millis(7400);

/// Keeps track of the start time for the live singing animation loop.
pub(in crate::screens::settings) struct LyricsPreview {
    started: Instant,
}

/// The lyrics settings the preview follows.
#[derive(Clone, Copy)]
struct Look {
    karaoke: bool,
    blur: bool,
    scripts: Option<RomanizationScripts>,
    preset: state::LyricsPreset,
}

impl LyricsPreview {
    pub(in crate::screens::settings) fn new(_cx: &mut Context<SettingsView>) -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

/// The Lyrics page's preview: sample lines singing on a live continuous loop,
/// dynamically showcasing word-rise, held swell, bloom, wave lift, or plain sweep
/// depending on the selected preset.
pub(in crate::screens::settings) fn preview(
    state: &LyricsPreview,
    view: &SettingsView,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
) -> Option<AnyElement> {
    window.request_animation_frame();
    let elapsed = state.started.elapsed();
    let position = Duration::from_nanos((elapsed.as_nanos() % LOOP_DURATION.as_nanos()) as u64);

    let height = (Layout::of(window, cx).width * ASPECT).round();
    let field = view.settings.read(cx).ambient().then(|| {
        let cover = view.previews.playing_or_picked(cx).large_cover(cx);
        view.previews.ambient(cover, cx)
    });
    // over the field the lines wear what fullscreen would, not the theme around them
    let theme = match &field {
        Some(field) => field.read(cx).theme(cx),
        None => *cx.theme(),
    };

    let settings = view.settings.read(cx);
    let look = Look {
        karaoke: settings.karaoke_lyrics(),
        blur: settings.blur_lyrics(),
        scripts: settings
            .romanized_lyrics()
            .then(|| settings.romanization_scripts()),
        preset: settings.lyrics_preset(),
    };
    let sizes = verse_sizes(false, settings.fullscreen_lyrics_scale(), &theme);
    let on = settings.lyrics_enabled();
    let sheet = div()
        .relative()
        .w_full()
        .h(height)
        .bg(theme.background)
        .children(field)
        .child(lines(look, sizes, height, &theme, position))
        .when(!on, |sheet| sheet.opacity(super::DIMMED));

    Some(super::framed(sheet, cx).into_any_element())
}

/// The sample lines around the one being sung, drawn the way the sheet draws them at the verse
/// and lane sizes in `sizes`, over the whole of a parent `height` tall.
fn lines(
    look: Look,
    sizes: (Pixels, Pixels),
    height: Pixels,
    theme: &Theme,
    position: Duration,
) -> Div {
    let lines = sample();
    let (size, lane) = sizes;
    let gap = verse_gap(size);
    let active_idx = lines
        .iter()
        .rposition(|line| line.start <= position)
        .unwrap_or(0);

    let verse_at = |index: usize| {
        let sung = index == active_idx;
        let distance = (index as isize - active_idx as isize).abs() as f32;
        let haze = match look.blur && !sung {
            true => (distance / 2.0).min(1.0).powf(HAZE),
            false => 0.,
        };
        verse(
            &lines[index],
            VerseLook {
                active: sung,
                size,
                lane,
                karaoke: look.karaoke,
                scripts: look.scripts,
                style: state::RomanizationStyle::Ruby,
                haze,
                preset: look.preset,
            },
            position,
            theme,
        )
    };
    let fade = size * VERSE_FADE;
    let anchor = height * ANCHOR;

    div()
        .absolute()
        .inset_0()
        .when(ui::effects(), |this| this.fade_edges(fade, fade))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(height - anchor)
                .pb(gap)
                .px(theme.metrics.inset)
                .flex()
                .flex_col()
                .gap(gap)
                .children((0..active_idx).map(verse_at)),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(anchor)
                .px(theme.metrics.inset)
                .flex()
                .flex_col()
                .gap(gap)
                .children((active_idx..lines.len()).map(verse_at)),
        )
}

/// The sample lines modeled directly from Oto Android's PreviewLyrics,
/// including a 900ms held note ("love") to showcase spring swell and bloom.
fn sample() -> Vec<LyricsLine> {
    vec![
        preview_line(
            Duration::from_millis(0),
            Duration::from_millis(1800),
            "if i am a stone",
            &[(0, 400), (400, 800), (800, 1200), (1200, 1400), (1400, 1800)],
            Voice::Lead,
        ),
        preview_line(
            Duration::from_millis(2000),
            Duration::from_millis(4400),
            "would you still love me?",
            &[
                (2000, 2400),
                (2400, 2800),
                (2800, 3200),
                (3200, 4100),
                (4100, 4400),
            ],
            Voice::Lead,
        ),
        preview_line(
            Duration::from_millis(4600),
            Duration::from_millis(7200),
            "of course, what do you mean my love",
            &[
                (4600, 4900),
                (4900, 5400),
                (5400, 5800),
                (5800, 6100),
                (6100, 6400),
                (6400, 6800),
                (6800, 7000),
                (7000, 7200),
            ],
            Voice::Duet,
        ),
    ]
}

fn preview_line(
    start: Duration,
    end: Duration,
    text: &str,
    windows: &[(u64, u64)],
    voice: Voice,
) -> LyricsLine {
    let raw_words: Vec<&str> = text.split_whitespace().collect();
    let word_count = raw_words.len();
    let words = raw_words
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            let (s_ms, e_ms) = windows.get(i).copied().unwrap_or((
                start.as_millis() as u64,
                end.as_millis() as u64,
            ));
            let trailing_space = if i + 1 < word_count {
                format!("{w} ")
            } else {
                w.to_owned()
            };
            LyricsWord {
                start: Duration::from_millis(s_ms),
                end: Duration::from_millis(e_ms),
                text: trailing_space,
            }
        })
        .collect();
    LyricsLine {
        start,
        end: Some(end),
        words: Some(words),
        text: text.to_owned(),
        voice,
        ..Default::default()
    }
}
