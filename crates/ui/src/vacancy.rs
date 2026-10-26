use gpui::prelude::*;
use gpui::{AnyElement, App, Div, SharedString, StyleRefinement, TextAlign, Window, div, svg};

use crate::label::vacant;
use crate::metrics::Text;
use crate::theme::ActiveTheme as _;

const GLYPH: f32 = 0.35;
const GLYPH_SIZE: f32 = 0.5;
const COMPACT_SIZE: f32 = 0.22;
const DETAIL: f32 = 0.8;
const WORDS: f32 = 2.5;

#[derive(IntoElement)]
pub struct Vacancy {
    base: Div,
    label: SharedString,
    detail: Option<SharedString>,
    icon: Option<SharedString>,
    action: Option<AnyElement>,
    compact: bool,
}

impl Vacancy {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            base: div(),
            label: label.into(),
            detail: None,
            icon: None,
            action: None,
            compact: false,
        }
    }

    /// A smaller glyph, for a narrow place like the sidebar.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// A second, quieter line under the label, for the reason behind an empty page.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl Styled for Vacancy {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Vacancy {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            mut base,
            label,
            detail,
            icon,
            action,
            compact,
        } = self;

        let theme = *cx.theme();
        let overrides = std::mem::take(base.style());
        let glyph = theme.metrics.cover
            * match compact {
                true => COMPACT_SIZE,
                false => GLYPH_SIZE,
            };

        let mut vacancy = base
            .flex()
            .flex_col()
            .w_full()
            .items_center()
            .justify_center()
            .when_some(icon, |this, icon| {
                this.child(
                    svg()
                        .path(icons::path(icon))
                        .size(glyph)
                        .mt(match compact {
                            true => theme.metrics.pad,
                            false => theme.metrics.inset,
                        })
                        .flex_none()
                        .text_color(theme.muted_foreground.opacity(GLYPH)),
                )
            })
            .child(match detail {
                Some(detail) => caption(label, detail, cx).into_any_element(),
                None => vacant(label, cx).into_any_element(),
            })
            .when_some(action, |this, action| this.child(action));

        vacancy.style().refine(&overrides);
        vacancy
    }
}

/// The label and its reason stacked, padded the way `vacant` pads its one line.
fn caption(label: SharedString, detail: SharedString, cx: &App) -> Div {
    let theme = cx.theme();

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .p(theme.metrics.pad * 2.)
        .max_w(theme.metrics.cover * WORDS)
        .text_align(TextAlign::Center)
        .child(
            div()
                .text_size(theme.text(Text::Label))
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .text_size(theme.text(Text::Small))
                .text_color(theme.muted_foreground.opacity(DETAIL))
                .child(detail),
        )
}
