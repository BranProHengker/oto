use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use gpui::prelude::*;
use gpui::{AnyElement, Context, FontWeight, SharedString, Window, div, img};
use i18n::t;
use state::{Channel, UpdateState};
use ui::{ActiveTheme as _, Button, Switch, Text};

use super::rows::{Row, outward, segmented};
use super::{Group, SettingsView, Slot};
use crate::shared::cells::relative_stamp;

/// How large the app icon stands, in covers.
const MARK_TILE: f32 = 0.5;
/// The app icon as this platform shows it, registered by the binary's asset source.
const APP_ICON: &str = "brand/sonora.svg";
const LICENSE_URL: &str = "https://www.gnu.org/licenses/gpl-3.0.html";
const SOURCE_URL: &str = "https://github.com/BranProHengker/oto";

impl SettingsView {
    pub(super) fn about_groups(&self) -> Vec<Group> {
        let itself = state::updates_itself()
            .then_some([Slot::AutoUpdate, Slot::Channel])
            .into_iter()
            .flatten();
        vec![
            Group::new("settings-group-updates")
                .rows([Slot::Version, Slot::Updates].into_iter().chain(itself)),
            Group::new("settings-group-project").rows([Slot::Log, Slot::License, Slot::Source]),
        ]
    }

    /// The version row: the build running, which for a nightly also names its commit, what
    /// the last look for updates found, and the button that looks again now.
    pub(super) fn version_row(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let version: SharedString = match state::commit() {
            Some(commit) => t!(
                "settings-version-commit",
                version = state::version(),
                commit = commit
            ),
            None => state::version().into(),
        };
        let updates = self.updates.read(cx);
        let checking = updates.checking();
        let status = match (updates.state(), checking) {
            (_, true) => t!("settings-update-status-checking"),
            (UpdateState::Offered(release), false) => t!(
                "settings-update-status-found",
                version = release.version.as_str()
            ),
            (UpdateState::Ready(release), false) => t!(
                "settings-update-status-ready",
                version = release.version.as_str()
            ),
            (UpdateState::Fetching(release), false) => {
                t!("update-downloading", name = release.version.as_str())
            }
            (UpdateState::Failed, false) => t!("update-failed"),
            (UpdateState::Quiet, false) if !updates.can_check() => {
                t!("settings-update-status-managed")
            }
            (UpdateState::Quiet, false) if updates.check_failed() => {
                t!("settings-update-status-failed")
            }
            (UpdateState::Quiet, false) if updates.found().is_some() => t!(
                "settings-update-status-found",
                version = updates.found().unwrap_or_default()
            ),
            (UpdateState::Quiet, false) => match updates.checked_at() {
                Some(at) => {
                    let when = relative_stamp(Some(at));
                    t!("settings-update-status-latest", when = when.as_ref())
                }
                None => t!("settings-update-status-never"),
            },
        };
        let can_check = updates.can_check()
            && !matches!(
                updates.state(),
                UpdateState::Fetching(_) | UpdateState::Ready(_)
            );

        let turn = {
            let mut spin = self.update_spin.borrow_mut();
            match checking {
                true => spin.start(),
                false => spin.stop(),
            }
            spin.turn(window, cx)
        };
        let check = Button::new("check-updates-now")
            .outline()
            .small()
            .icon("icons/refresh-cw.svg")
            .turned(turn)
            .label(t!("settings-check-updates"))
            .disabled(checking || !can_check)
            .on_click(cx.listener(|this, _, _, cx| {
                this.updates.update(cx, |updates, cx| updates.check(cx));
            }));

        Row::new(
            t!("settings-version-title", version = version.as_ref()),
            check.into_any_element(),
        )
        .status(status)
        .into_any_element()
    }

    pub(super) fn updates_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.settings.read(cx).check_updates();

        self.row(
            t!("settings-auto-check"),
            Switch::new("check-updates", on)
                .name(t!("settings-auto-check"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.settings
                        .update(cx, |settings, cx| settings.set_check_updates(!on, cx));
                }))
                .into_any_element(),
        )
    }

    pub(super) fn auto_update_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.settings.read(cx).auto_update();

        self.row(
            t!("settings-auto-update"),
            Switch::new("auto-update", on)
                .name(t!("settings-auto-update"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.settings
                        .update(cx, |settings, cx| settings.set_auto_update(!on, cx));
                }))
                .into_any_element(),
        )
    }

    pub(super) fn channel_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.settings.read(cx).update_channel();

        let choices = segmented(
            "update-channel",
            Channel::ALL.map(|channel| (channel, channel.label())),
            current,
            cx.listener(|this, channel: &Channel, _, cx| {
                let channel = *channel;
                this.settings
                    .update(cx, |settings, cx| settings.set_update_channel(channel, cx));
            }),
        );

        self.row(
            t!("settings-update-channel"),
            choices
                .name(t!("settings-update-channel"))
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
                    .id("settings-about-name")
                    .role(gpui::accesskit::Role::Heading)
                    .aria_level(2)
                    .aria_label(t!("common-sonora"))
                    .text_size(theme.text(Text::Title))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("common-sonora")),
            )
    }

    pub(super) fn notice(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();

        div()
            .id("settings-notice")
            .role(gpui::accesskit::Role::Paragraph)
            .aria_label(t!("settings-notice"))
            .text_color(theme.muted_foreground)
            .text_size(theme.text(Text::Small))
            .child(t!("settings-notice"))
    }
}

/// Notepad on Windows refuses a file that Oto still writes to, so the button opens a copy.
/// `File::open` shares write access, and `fs::copy` documents no share mode.
fn snapshot(log: &Path) -> io::Result<PathBuf> {
    let copy = log.with_file_name("oto.copy.log");
    io::copy(&mut File::open(log)?, &mut File::create(&copy)?)?;
    Ok(copy)
}
