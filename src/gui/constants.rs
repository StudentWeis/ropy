use gpui::{px, size};

pub const DEFAULT_WINDOW_WIDTH_PX: f32 = 400.0;
pub const DEFAULT_WINDOW_HEIGHT_PX: f32 = 600.0;

pub const fn default_window_size() -> gpui::Size<gpui::Pixels> {
    size(px(DEFAULT_WINDOW_WIDTH_PX), px(DEFAULT_WINDOW_HEIGHT_PX))
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::*;

    #[test]
    fn test_default_window_size_returns_expected_dimensions() {
        let size = default_window_size();

        assert_eq!(size.width, px(DEFAULT_WINDOW_WIDTH_PX));
        assert_eq!(size.height, px(DEFAULT_WINDOW_HEIGHT_PX));
    }
}
