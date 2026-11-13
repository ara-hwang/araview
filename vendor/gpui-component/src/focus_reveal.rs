// AraView fork: keyboard focus asks enclosing scroll areas to reveal the control.
use std::{cell::Cell, rc::Rc};

use gpui::{App, Bounds, ElementId, FocusHandle, Pixels, Window, px};

pub(crate) fn on_focus(
    id: ElementId,
    focus: FocusHandle,
    window: &mut Window,
    cx: &mut App,
) -> impl FnOnce(Bounds<Pixels>, &mut Window, &mut App) + 'static {
    let was_focused = window
        .use_keyed_state((id, "focus-reveal"), cx, |_, _| Rc::new(Cell::new(false)))
        .read(cx)
        .clone();
    move |bounds, window, _| {
        let focused = focus.is_focused(window);
        if !was_focused.replace(focused) && focused && window.last_input_was_keyboard() {
            window.request_autoscroll(bounds.dilate(px(4.)));
        }
    }
}
