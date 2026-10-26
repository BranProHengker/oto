<div align="center">

<img src="./.github/readme-icon.svg" alt="Oto Logo" width="80" height="80">

<h1>Oto</h1>

<p align="center">
  A fast, native music streaming client and local player built with Rust and GPUI.
</p>

[![License](https://img.shields.io/badge/license-GPL--3.0-blue.svg?style=flat-square)](./COPYING)
[![Rust](https://img.shields.io/badge/rust-2024_edition-orange.svg?style=flat-square)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/GUI-GPUI-purple.svg?style=flat-square)](https://github.com/zed-industries/zed)

</div>

> [!NOTE]
> **Oto** is an independent fork of [Sonora](https://github.com/sonorahq/sonora). It retains full compatibility with Sonora's native playback engine while adding tailored improvements:
> - **Dual-Line Synced Lyrics**: Smart detection and merging of twin-timestamp `.lrc` files (Japanese Kanji/Kana as primary line + Romaji furigana sublane).
> - **Clean UI & Settings**: Streamlined interface without upstream updater clutter or redundant links.
> - **Linux Tray & Desktop Integration**: Native StatusNotifierItem (SNI) tray support optimized for Hyprland, Caelestia shell, and Waybar.
> - **Custom Discord Rich Presence**: Real-time status reporting with direct asset fallbacks for local and streaming playback.

---

## Features

- **Streaming & Local**: Apple Music, Spotify, YouTube Music, Deezer, Tidal, Subsonic/Navidrome, and offline local files.
- **Pure Native UI**: Rendered with GPUI (Zed's GPU-accelerated UI framework) — zero Electron, zero WebKit, ultra-low memory usage (~170 MB RSS).
- **Karaoke & Synced Lyrics**: Time-synced scrolling lyrics, background vocals, furigana/romaji support, and multi-provider lyrics fallback.
- **Audio Engine**: Gapless playback, audio loudness normalization, parametric equalizer, and sleep timer.
- **Audio Scrobbler**: Built-in support for LastFM, ListenBrainz, LibreFM, and Maloja.
- **Extensive Theming**: Dynamic adaptive palettes, blur/vibrancy, font adjustments, and interchangeable icon packs.

---

## Building from Source

### Prerequisites

- **Rust toolchain** (nightly/stable with Edition 2024 support)
- **C/C++ compiler** (for Widevine CDM parsing & system bindings)
- **Mold linker** (recommended on Linux for ultra-fast link times)
- Sound server headers (`pipewire-alsa` or `pulseaudio-alsa`)

### Build & Run

```sh
# Clone this repository
git clone https://github.com/BranProHengker/oto.git
cd oto

# Run directly
cargo run --locked --package sonora --bin oto

# Build optimized release binary
cargo build --release --locked --package sonora --bin oto
```

The resulting binary will be at `target/release/oto` (or `target/debug/oto`).

---

## Credits & Upstream

Oto builds upon incredible open-source foundations:

- [Sonora](https://github.com/sonorahq/sonora) — The original native streaming music player.
- [Zed](https://github.com/zed-industries/zed) — Creators of [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui), the blazing fast GPU rendering framework.
- [librespot](https://github.com/librespot-org/librespot) — Spotify playback & metadata integration.
- [ytmusic-rs](https://github.com/sonorahq/ytmusic-rs) — YouTube Music API implementation.

---

## License

Oto is free software licensed under the [GNU General Public License version 3 or later](COPYING).
