use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, div, img};
use i18n::t;
use ui::{ActiveTheme as _, Switch, Text};

use super::rows::outward;
use super::{Group, SettingsView, Slot};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// How large the app icon stands, in covers.
const MARK_TILE: f32 = 0.5;
/// The app icon as this platform shows it, registered by the binary's asset source.
const APP_ICON: &str = "brand/sonora.svg";
const LICENSE_URL: &str = "https://www.gnu.org/licenses/gpl-3.0.html";
const SOURCE_URL: &str = "https://github.com/sonorahq/sonora";

impl SettingsView {
    pub(super) fn about_groups(&self) -> Vec<Group> {
        vec![Group::untitled().rows([
            Slot::Version,
            Slot::Log,
            Slot::License,
        ])]
    }

    pub(super) fn version_row(&self) -> AnyElement {
        self.row(
            t!("settings-version"),
            div().child(VERSION).into_any_element(),
        )
    }

    pub(super) fn updates_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.settings.read(cx).check_updates();

        self.row(
            t!("settings-check-updates"),
            Switch::new("check-updates", on)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.settings
                        .update(cx, |settings, cx| settings.set_check_updates(!on, cx));
                }))
                .into_any_element(),
        )
    }

    /// The row that opens a copy of the current log file in whatever the system reads text
    /// with. It is disabled when the platform names no state or cache folder to log into.
    pub(super) fn log_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let path = state::log_file();

        outward("open-log", t!("settings-log-open"), None, cx)
            .disabled(path.is_none())
            .on_click(move |_, _, _| {
                let Some(path) = &path else { return };
                if let Err(error) = snapshot(path).and_then(open::that_detached) {
                    log::warn!(
                        "settings: cannot open a copy of {}: {error}",
                        path.display()
                    );
                }
            })
            .into_any_element()
    }

    pub(super) fn license_row(&self, cx: &mut Context<Self>) -> AnyElement {
        outward(
            "license",
            t!("settings-license"),
            Some(t!("settings-license-short")),
            cx,
        )
        .on_click(|_, _, cx| cx.open_url(LICENSE_URL))
        .into_any_element()
    }

    pub(super) fn source_row(&self, cx: &mut Context<Self>) -> AnyElement {
        outward("source", t!("settings-source"), None, cx)
            .on_click(|_, _, cx| cx.open_url(SOURCE_URL))
            .into_any_element()
    }

    /// Opens About with the app icon beside its name, aligned with the settings below. The icon
    /// is the one this platform shows, in its own colours whatever the theme.
    pub(super) fn mark(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let tile = theme.metrics.cover * MARK_TILE;

        div()
            .flex()
            .items_center()
            .gap_5()
            .child(img(APP_ICON).size(tile).flex_none())
            .child(
                div()
                    .text_size(theme.text(Text::Title))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("settings-about-name")),
            )
    }

    pub(super) fn notice(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();

        div()
            .text_color(theme.muted_foreground)
            .text_size(theme.text(Text::Small))
            .child(t!("settings-notice"))
    }
}

/// Notepad on Windows refuses a file that Sonora still writes to, so the button opens a copy.
/// `File::open` shares write access, and `fs::copy` documents no share mode.
fn snapshot(log: &Path) -> io::Result<PathBuf> {
    let copy = log.with_file_name("sonora.copy.log");
    io::copy(&mut File::open(log)?, &mut File::create(&copy)?)?;
    Ok(copy)
}
