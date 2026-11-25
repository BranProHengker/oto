use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, Pixels, Render, StyleRefinement, Window, div, px};
use state::{AppSettings, Playback, Queue, SideTab, Sonora};
use ui::{ActiveTheme as _, MIN_CONTENT, Panel, Room, Side};

use crate::chrome::{Aside, Members};

const MIN_WIDTH: Pixels = px(240.);
const MAX_WIDTH: Pixels = px(560.);

fn fills_content(width: Pixels) -> bool {
    !Room::of(width).fits(Room::Wide)
}

/// The right sidebar: the queue and the lyrics, which can be shown individually or side-by-side
/// in dual preview mode, and the members of the session or Jam.
pub(crate) struct SidebarRight {
    queue_aside: Entity<Aside>,
    lyrics_aside: Entity<Aside>,
    members: Entity<Members>,
    settings: Entity<AppSettings>,
    width: Pixels,
    open: bool,
    order: Vec<SideTab>,
    /// Whether the sidebar still slides away after closing, so it keeps drawing until it is gone.
    lingering: bool,
    /// How many times the listener has opened or closed the sidebar. The page springs aside only
    /// when this moves, so a window resize never sets the sidebar sliding.
    slides: usize,
}

impl SidebarRight {
    pub(crate) fn new(
        queue: Entity<Queue>,
        playback: Entity<Playback>,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings = Sonora::global(cx).settings.clone();
        let width = px(settings.read(cx).sidebar_right_width()).clamp(MIN_WIDTH, MAX_WIDTH);
        let open = settings.read(cx).sidebar_right_open();
        let primary = settings.read(cx).sidebar_right_tab();
        let secondary = settings.read(cx).sidebar_right_secondary();
        let dual = settings.read(cx).dual_preview();

        let queue_aside =
            cx.new(|cx| Aside::new(queue.clone(), playback.clone(), SideTab::Queue, cx));
        let lyrics_aside = cx.new(|cx| Aside::new(queue, playback, SideTab::Lyrics, cx));
        let members = cx.new(Members::new);

        let mut order = Vec::new();
        if open {
            if primary == SideTab::Members {
                order.push(SideTab::Members);
            } else if dual {
                if let Some(sec) = secondary
                    && sec != primary
                    && sec != SideTab::Members
                {
                    order.push(sec);
                }
                order.push(primary);
            } else {
                order.push(primary);
            }
        }

        cx.observe(&settings, |this, settings, cx| {
            if !settings.read(cx).lyrics_enabled() {
                let had_lyrics = this.order.contains(&SideTab::Lyrics);
                this.order.retain(|&tab| tab != SideTab::Lyrics);
                if this.order.is_empty() && this.open {
                    this.order.push(SideTab::Queue);
                }
                if had_lyrics {
                    this.remember(cx);
                    cx.notify();
                }
            }
        })
        .detach();

        Self {
            queue_aside,
            lyrics_aside,
            members,
            settings,
            width,
            open,
            order,
            lingering: false,
            slides: 0,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open && !self.order.is_empty()
    }

    pub(crate) fn slides(&self) -> usize {
        self.slides
    }

    /// Keeps a closed sidebar drawn while it slides away beside the page.
    pub(crate) fn linger(&mut self, lingering: bool, cx: &mut Context<Self>) {
        if self.lingering != lingering {
            self.lingering = lingering;
            cx.notify();
        }
    }

    /// How wide the sidebar is when it shows, whether or not it shows now.
    pub(crate) fn breadth(&self) -> Pixels {
        if self.order.len() > 1 {
            self.width * 2.
        } else {
            self.width
        }
    }

    pub(crate) fn available(window: &Window) -> bool {
        !fills_content(window.viewport_size().width)
    }

    pub(crate) fn covers_content(&self, _window: &Window) -> bool {
        false
    }

    pub(crate) fn occupied_width(&self, window: &Window) -> Pixels {
        match self.open && !self.order.is_empty() && Self::available(window) {
            false => Pixels::ZERO,
            true => {
                if self.order.len() > 1 {
                    self.width * 2.
                } else {
                    self.width
                }
            }
        }
    }

    /// The width the left sidebar has to leave free for this one. It counts an open sidebar
    /// even while the window is too narrow to show it, so the left sidebar collapses first and
    /// stays collapsed when this one hides in turn.
    pub(crate) fn reserved_width(&self) -> Pixels {
        match self.open && !self.order.is_empty() {
            false => Pixels::ZERO,
            true => {
                if self.order.len() > 1 {
                    self.width * 2.
                } else {
                    self.width
                }
            }
        }
    }

    pub(crate) fn toggle(&mut self, cx: &mut Context<Self>) {
        self.slides += 1;
        if self.open && !self.order.is_empty() {
            self.close(cx);
        } else {
            let primary = self.settings.read(cx).sidebar_right_tab();
            self.show(primary, cx);
        }
    }

    pub(crate) fn show(&mut self, tab: SideTab, cx: &mut Context<Self>) {
        if tab == SideTab::Lyrics && !self.settings.read(cx).lyrics_enabled() {
            return;
        }

        if tab == SideTab::Members {
            if self.open && self.order.contains(&SideTab::Members) {
                self.close(cx);
                return;
            }
            if !self.open {
                self.slides += 1;
            }
            self.order = vec![SideTab::Members];
            self.open = true;
            self.remember(cx);
            cx.notify();
            return;
        }

        let dual = self.settings.read(cx).dual_preview();

        if !dual {
            if self.open && self.order.contains(&tab) {
                self.close(cx);
                return;
            }
            if !self.open {
                self.slides += 1;
            }
            self.order = vec![tab];
            self.open = true;
            match tab {
                SideTab::Queue => self.queue_aside.update(cx, |aside, cx| aside.show(tab, cx)),
                SideTab::Lyrics => self.lyrics_aside.update(cx, |aside, cx| aside.show(tab, cx)),
                _ => {}
            }
            self.remember(cx);
            cx.notify();
            return;
        }

        // Dual preview enabled:
        // If tab is already open: toggle it off!
        if self.open && self.order.contains(&tab) {
            self.order.retain(|&t| t != tab);
            if self.order.is_empty() {
                self.close(cx);
                return;
            }
            self.remember(cx);
            cx.notify();
            return;
        }

        // Tab is NOT open yet:
        // Remove Members if switching to queue/lyrics
        self.order.retain(|&t| t != SideTab::Members);

        if self.order.is_empty() || !self.open {
            if !self.open {
                self.slides += 1;
            }
            self.order = vec![tab];
            self.open = true;
        } else {
            // Already have a panel open!
            // First panel stays on the right, new panel appears on the left (index 0):
            self.order.insert(0, tab);
            if self.order.len() > 2 {
                self.order.truncate(2);
            }
        }

        match tab {
            SideTab::Queue => self.queue_aside.update(cx, |aside, cx| aside.show(tab, cx)),
            SideTab::Lyrics => self.lyrics_aside.update(cx, |aside, cx| aside.show(tab, cx)),
            _ => {}
        }
        self.remember(cx);
        cx.notify();
    }

    /// Shows `tab`, and leaves the sidebar open when it already shows it, where `show` would
    /// close it.
    pub(crate) fn reveal(&mut self, tab: SideTab, cx: &mut Context<Self>) {
        if self.open && self.order.contains(&tab) {
            return;
        }
        self.show(tab, cx);
    }

    pub(crate) fn close(&mut self, cx: &mut Context<Self>) {
        self.queue_aside.update(cx, |aside, cx| aside.dismiss(cx));
        self.lyrics_aside.update(cx, |aside, cx| aside.dismiss(cx));
        if self.open {
            self.slides += 1;
        }
        self.open = false;
        self.order.clear();
        self.remember(cx);
        cx.notify();
    }

    fn remember(&self, cx: &mut Context<Self>) {
        let open = self.open && !self.order.is_empty();
        let (primary, secondary) = match self.order.as_slice() {
            [sec, prim, ..] => (*prim, Some(*sec)),
            [prim] => (*prim, None),
            [] => (self.settings.read(cx).sidebar_right_tab(), None),
        };
        self.settings.update(cx, |settings, cx| {
            settings.set_sidebar_right_open(open, cx);
            settings.set_sidebar_right_tab(primary, cx);
            settings.set_sidebar_right_secondary(secondary, cx);
        });
    }

    fn persist(&self, cx: &mut Context<Self>) {
        let width = self.width / px(1.);
        self.settings.update(cx, |settings, cx| {
            settings.set_sidebar_right_width(width, cx)
        });
    }

    fn render_tab(&self, tab: SideTab) -> AnyElement {
        match tab {
            SideTab::Members => self.members.clone().into_any_element(),
            SideTab::Queue => self
                .queue_aside
                .clone()
                .cached(StyleRefinement::default().size_full())
                .into_any_element(),
            SideTab::Lyrics => self
                .lyrics_aside
                .clone()
                .cached(StyleRefinement::default().size_full())
                .into_any_element(),
        }
    }
}

impl Render for SidebarRight {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if (!self.is_open() && !self.lingering) || !Self::available(window) {
            return div().into_any_element();
        }

        let theme = *cx.theme();
        let dual = self.order.len() > 1;
        let total_width = if dual { self.width * 2. } else { self.width };
        let min_w = if dual { MIN_WIDTH * 2. } else { MIN_WIDTH };
        let max_w = if dual { MAX_WIDTH * 2. } else { MAX_WIDTH };

        Panel::new("sidebar-right", Side::Right, total_width)
            .limits(min_w, max_w)
            .reach(super::cap(min_w, max_w, MIN_CONTENT, window))
            .grip_above(super::Chrome::bottom(cx))
            .on_resize(cx.listener(|this, width: &Pixels, _, cx| {
                if this.order.len() > 1 {
                    this.width = (*width / 2.).clamp(MIN_WIDTH, MAX_WIDTH);
                } else {
                    this.width = (*width).clamp(MIN_WIDTH, MAX_WIDTH);
                }
                this.persist(cx);
                cx.notify();
            }))
            .when(!theme.transparent, |this| this.bg(theme.sidebar))
            .border_color(gpui::transparent_black())
            .child(match self.order.as_slice() {
                [left, right, ..] => {
                    let left_view = self.render_tab(*left);
                    let right_view = self.render_tab(*right);
                    div()
                        .flex()
                        .flex_row()
                        .size_full()
                        .child(
                            div()
                                .flex_1()
                                .h_full()
                                .min_w_0()
                                .child(left_view),
                        )
                        .child(div().w_px().h_full().bg(theme.border))
                        .child(
                            div()
                                .flex_1()
                                .h_full()
                                .min_w_0()
                                .child(right_view),
                        )
                        .into_any_element()
                }
                [single] => self.render_tab(*single),
                [] => div().into_any_element(),
            })
            .into_any_element()
    }
}
