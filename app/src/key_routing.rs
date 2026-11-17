//! App-level input priority, independent of key binding syntax or GPUI state.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyRoute {
    Blocked,
    Grid,
    Viewer,
}

/// Modal and text-input handlers own their events before either image-viewing context.
pub(crate) fn key_route(dialog: bool, input: bool, grid: bool) -> KeyRoute {
    if dialog || input {
        KeyRoute::Blocked
    } else if grid {
        KeyRoute::Grid
    } else {
        KeyRoute::Viewer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_and_input_always_win_over_grid_and_viewer() {
        for grid in [false, true] {
            assert_eq!(key_route(true, false, grid), KeyRoute::Blocked);
            assert_eq!(key_route(false, true, grid), KeyRoute::Blocked);
            assert_eq!(key_route(true, true, grid), KeyRoute::Blocked);
        }
    }

    #[test]
    fn unobstructed_grid_excludes_viewer_shortcuts() {
        assert_eq!(key_route(false, false, true), KeyRoute::Grid);
        assert_eq!(key_route(false, false, false), KeyRoute::Viewer);
    }
}
