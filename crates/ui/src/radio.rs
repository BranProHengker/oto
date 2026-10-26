use gpui::prelude::*;
use gpui::{App, Div, ElementId, StyleRefinement, Window, div, px};

use crate::motion::{Motion, Motioned as _, Movement, mix};
use crate::theme::ActiveTheme as _;

const SCALE: f32 = 0.6;
/// How much of the mark's width the hole in the middle takes.
const HOLE: f32 = 0.34;

/// The round mark that says which one of a set is chosen: filled in the accent colour with a
/// hole punched in the middle when it is, an empty ring when it is not. It takes no clicks of
/// its own, so the row around it is what the pointer presses.
#[derive(IntoElement)]
pub struct Radio {
    id: ElementId,
    base: Div,
    selected: bool,
}

impl Radio {
    #[track_caller]
    pub fn new(id: impl Into<ElementId>, selected: bool) -> Self {
        Self {
            id: id.into(),
            base: div(),
            selected,
        }
    }
}

impl Styled for Radio {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            mut base,
            selected,
        } = self;
        let theme = *cx.theme();
        let side = px((theme.metrics.control_small / px(1.) * SCALE).round());
        let overrides = std::mem::take(base.style());

        let (fill_was, fill_is) = match selected {
            true => (theme.primary.opacity(0.), theme.primary),
            false => (theme.primary, theme.primary.opacity(0.)),
        };
        let (edge_was, edge_is) = match selected {
            true => (theme.border, theme.primary),
            false => (theme.primary, theme.border),
        };
        let (hole_was, hole_is) = match selected {
            true => (
                theme.primary_foreground.opacity(0.),
                theme.primary_foreground,
            ),
            false => (
                theme.primary_foreground,
                theme.primary_foreground.opacity(0.),
            ),
        };

        let movement = window.use_keyed_state((id, "movement"), cx, |_, _| Movement::new(selected));
        let animates = movement.update(cx, |movement, _| movement.turning(selected));
        let hole = px((side / px(1.) * HOLE).round());
        let dot = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(side)
            .rounded_full()
            .border_1();
        let dot = match animates {
            true => dot
                .motion(
                    ("dot", usize::from(selected)),
                    Motion::Control,
                    move |dot, t| {
                        dot.bg(mix(fill_was, fill_is, t))
                            .border_color(mix(edge_was, edge_is, t))
                            .child(
                                div()
                                    .size(hole)
                                    .rounded_full()
                                    .bg(mix(hole_was, hole_is, t)),
                            )
                    },
                )
                .into_any_element(),
            false => dot
                .bg(fill_is)
                .border_color(edge_is)
                .child(div().size(hole).rounded_full().bg(hole_is))
                .into_any_element(),
        };

        let mut radio = base
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .child(dot);
        radio.style().refine(&overrides);
        radio
    }
}
