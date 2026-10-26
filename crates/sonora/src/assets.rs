use std::borrow::Cow;

use anyhow::Result;
use gpui::{App, AssetSource, SharedString};

include!(concat!(env!("OUT_DIR"), "/fonts.rs"));

/// Where About asks for the app icon, and the icon this platform shows for the app.
const APP_ICON: &str = "brand/sonora.svg";
#[cfg(target_os = "macos")]
const APP_ICON_SVG: &[u8] = include_bytes!("../../../assets/macos/sonora.svg");
#[cfg(target_os = "windows")]
const APP_ICON_SVG: &[u8] = include_bytes!("../../../assets/windows/sonora.svg");
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const APP_ICON_SVG: &[u8] = include_bytes!("../../../assets/linux/sonora.svg");

pub struct Assets;

impl Assets {
    pub fn load_fonts(&self, cx: &App) -> Result<()> {
        let embedded = FONTS
            .iter()
            .map(|(_, bytes)| Cow::Borrowed(*bytes))
            .collect();

        cx.text_system().add_fonts(embedded)
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == APP_ICON {
            return Ok(Some(Cow::Borrowed(APP_ICON_SVG)));
        }
        if let Some(bytes) = icons::asset(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        if let Some((_, bytes)) = FONTS.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        log::warn!("assets: {path} is not registered");
        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut listed = icons::Assets.list(path)?;
        listed.extend(
            FONTS
                .iter()
                .filter(|(name, _)| name.starts_with(path))
                .map(|(name, _)| SharedString::new_static(name)),
        );
        Ok(listed)
    }
}
