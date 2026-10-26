use gpui::div;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString};
use ui::Kbd;

use super::{Group, SettingsView, Slot};

impl SettingsView {
    pub(super) fn shortcuts_groups(&self) -> Vec<Group> {
        vec![
            Group::new("settings-shortcuts-playback", "icons/play.svg").rows([
                Slot::ShortcutPlayback,
                Slot::ShortcutNext,
                Slot::ShortcutPrevious,
                Slot::ShortcutShuffle,
                Slot::ShortcutRepeat,
                Slot::ShortcutVolumeUp,
                Slot::ShortcutVolumeDown,
                Slot::ShortcutVolumeWheel,
                Slot::ShortcutFullscreen,
            ]),
            Group::new("settings-shortcuts-navigation", "icons/arrow-right-left.svg").rows([
                Slot::ShortcutSearch,
                Slot::ShortcutFilter,
                Slot::ShortcutQueue,
                Slot::ShortcutLyrics,
                Slot::ShortcutBack,
                Slot::ShortcutForward,
                Slot::ShortcutRefresh,
                Slot::ShortcutSettings,
            ]),
            Group::new("settings-shortcuts-window", "icons/maximize.svg").rows([
                Slot::ShortcutFullscreenWindow,
                Slot::ShortcutIncreaseFont,
                Slot::ShortcutDecreaseFont,
                Slot::ShortcutResetFont,
                Slot::ShortcutDismiss,
                Slot::ShortcutQuit,
            ]),
            Group::new("settings-shortcuts-equalizer", "icons/undo-2.svg").rows([
                Slot::ShortcutUndo,
                Slot::ShortcutRedo,
            ]),
            Group::new("settings-shortcuts-table", "icons/list.svg").rows([
                Slot::ShortcutSelectNext,
                Slot::ShortcutSelectPrevious,
                Slot::ShortcutActivate,
                Slot::ShortcutRemove,
                Slot::ShortcutDeselect,
            ]),
        ]
    }

    pub(super) fn shortcut_row(&self, title: SharedString, keystroke: &'static str) -> AnyElement {
        self.row(title, Kbd::new(keystroke).into_any_element())
    }

    pub(super) fn shortcut_wheel_row(&self, title: SharedString) -> AnyElement {
        let control = div()
            .flex()
            .items_center()
            .gap_1()
            .child(Kbd::new("secondary"))
            .child(Kbd::new("wheel"));
        self.row(title, control.into_any_element())
    }
}
