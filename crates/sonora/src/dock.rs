#[cfg(target_os = "macos")]
pub fn show(shown: bool) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    let Some(mtm) = MainThreadMarker::new() else {
        return log::warn!("dock: activation policy changed off the main thread");
    };
    let policy = match shown {
        true => NSApplicationActivationPolicy::Regular,
        false => NSApplicationActivationPolicy::Accessory,
    };
    NSApplication::sharedApplication(mtm).setActivationPolicy(policy);
}

#[cfg(not(target_os = "macos"))]
pub fn show(_shown: bool) {}

/// The right-click menu on the Dock icon: transport plus shuffle and repeat, mirroring the tray.
#[cfg(target_os = "macos")]
pub fn menu(shown: &crate::tray::Shown, cx: &mut gpui::App) {
    use gpui::MenuItem;
    use input::{SongNext, SongPrevious, TogglePlayback, ToggleRepeat, ToggleShuffle};

    cx.set_dock_menu(vec![
        MenuItem::action(shown.toggle.clone(), TogglePlayback),
        MenuItem::action(shown.previous.clone(), SongPrevious),
        MenuItem::action(shown.next.clone(), SongNext),
        MenuItem::separator(),
        MenuItem::action(shown.shuffle.clone(), ToggleShuffle).checked(shown.shuffle_on),
        MenuItem::action(shown.repeat.clone(), ToggleRepeat).checked(shown.repeat_on),
    ]);
}

#[cfg(not(target_os = "macos"))]
pub fn menu(_shown: &crate::tray::Shown, _cx: &mut gpui::App) {}
