pub(crate) mod fullscreen;
pub(crate) mod workspace;

use gpui::{AnyView, App, KeyContext};

use crate::chrome::TitleBarOptions;

pub(crate) trait Shell {
    fn title_bar(&self, content: Option<AnyView>, cx: &App) -> TitleBarOptions;
}

/// The key context a shell sets on its root, built from the bare name. A shell renders every frame
/// an animation in it runs, and parsing the name each time would copy it every frame.
pub(crate) fn shell_context(name: &'static str) -> KeyContext {
    let mut context = KeyContext::default();
    context.add(name);
    context
}
