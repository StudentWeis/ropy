use std::path::Path;

use super::super::color::parse_clipboard_color;
use crate::{
    config::LayoutMode,
    repository::{ClipboardRecord, models::ContentType},
    utils::deserialize_file_paths,
};

pub(super) const GRID_COLUMN_COUNT: usize = 2;
// Mirror constant in `f32` form so masonry layout math avoids per-call casts.
#[expect(
    clippy::cast_precision_loss,
    reason = "GRID_COLUMN_COUNT is a tiny compile-time literal"
)]
pub(super) const GRID_COLUMN_COUNT_F32: f32 = GRID_COLUMN_COUNT as f32;
pub(super) const GRID_CONTENT_PREVIEW_LIMIT: usize = 96;
pub(super) const GRID_CONTENT_PREVIEW_MAX_LINES: usize = 4;
pub(super) const GRID_CARD_MIN_HEIGHT: f32 = 96.0;
pub(super) const GRID_CARD_MAX_HEIGHT: f32 = 144.0;
pub(super) const GRID_IMAGE_MAX_HEIGHT: f32 = 80.0;
pub(super) const GRID_COLOR_SWATCH_HEIGHT: f32 = 48.0;
pub(super) const GRID_COLOR_SWATCH_GAP: f32 = 6.0;
const GRID_ESTIMATED_CARD_CHROME_HEIGHT: f32 = 40.0;
pub(super) const GRID_ESTIMATED_TEXT_LINE_HEIGHT: f32 = 16.0;
const GRID_ESTIMATED_FILE_TITLE_HEIGHT: f32 = 16.0;
const GRID_ESTIMATED_FILE_DETAIL_LINE_HEIGHT: f32 = 15.0;
const GRID_ESTIMATED_TEXT_LINE_WIDTH_UNITS: f32 = 11.0;
pub(super) const LIST_CONTENT_PREVIEW_LIMIT: usize = 80;
pub(super) const LIST_CONTENT_PREVIEW_MAX_LINES: usize = 3;
pub(super) const LIST_COLOR_SWATCH_SIZE_PX: f32 = 16.0;
pub(super) const TOOLTIP_CONTENT_PREVIEW_LIMIT: usize = 500;
pub(super) const TOOLTIP_CONTENT_PREVIEW_MAX_LINES: usize = 10;

pub(in crate::gui::board) const fn visible_list_len(
    record_count: usize,
    layout_mode: LayoutMode,
) -> usize {
    match layout_mode {
        LayoutMode::List => record_count,
        LayoutMode::Grid => {
            if record_count == 0 {
                0
            } else {
                record_count.div_ceil(GRID_COLUMN_COUNT)
            }
        }
    }
}

pub(in crate::gui::board) const fn list_row_for_selected_index(
    selected_index: usize,
    layout_mode: LayoutMode,
) -> usize {
    match layout_mode {
        LayoutMode::List => selected_index,
        LayoutMode::Grid => selected_index / GRID_COLUMN_COUNT,
    }
}

/// Configuration for [`truncate`].
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct TruncateOptions {
    /// Optional upper bound on the number of lines to keep. `None` applies no
    /// line limit, so only the character limit trims the result.
    max_lines: Option<usize>,
}

impl TruncateOptions {
    pub(super) const fn with_max_lines(max_lines: usize) -> Self {
        Self {
            max_lines: Some(max_lines),
        }
    }
}

/// Truncate `content` to at most `char_limit` characters and, when requested,
/// at most `options.max_lines` lines. Appends an ellipsis when either bound
/// causes the result to drop trailing content.
pub(super) fn truncate(content: &str, char_limit: usize, options: TruncateOptions) -> String {
    let Some(max_lines) = options.max_lines else {
        return if content.chars().count() > char_limit {
            format!(
                "{}...",
                content.chars().take(char_limit).collect::<String>()
            )
        } else {
            content.to_string()
        };
    };

    let lines: Vec<&str> = content.lines().take(max_lines).collect();
    let line_limited_content = lines.join("\n");

    if line_limited_content.chars().count() > char_limit {
        format!(
            "{}...",
            line_limited_content
                .chars()
                .take(char_limit)
                .collect::<String>()
        )
    } else if content.lines().count() > max_lines {
        format!("{line_limited_content}...")
    } else {
        line_limited_content
    }
}

fn estimated_text_units(content: &str) -> f32 {
    content
        .chars()
        .map(|ch| {
            if ch.is_ascii_whitespace() {
                0.35
            } else if ch.is_ascii_punctuation() {
                0.5
            } else if ch.is_ascii() {
                0.65
            } else {
                1.1
            }
        })
        .sum()
}

fn estimated_wrapped_line_count(content: &str, max_lines: usize) -> usize {
    let line_count = content
        .lines()
        .take(max_lines)
        .map(|line| {
            if line.is_empty() {
                1
            } else {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = ".max(1.0) keeps the value in [1.0, max_lines), well within usize"
                )]
                let lines = (estimated_text_units(line) / GRID_ESTIMATED_TEXT_LINE_WIDTH_UNITS)
                    .ceil()
                    .max(1.0) as usize;
                lines
            }
        })
        .sum::<usize>();

    line_count.clamp(1, max_lines)
}

pub(super) fn estimated_grid_text_lines(content: &str) -> usize {
    estimated_wrapped_line_count(
        &truncate(
            content.trim_start(),
            GRID_CONTENT_PREVIEW_LIMIT,
            TruncateOptions::with_max_lines(GRID_CONTENT_PREVIEW_MAX_LINES),
        ),
        GRID_CONTENT_PREVIEW_MAX_LINES,
    )
}

pub(super) fn estimated_grid_color_body_height(content: &str) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "line count is bounded by GRID_CONTENT_PREVIEW_MAX_LINES (4)"
    )]
    let line_count = estimated_grid_text_lines(content) as f32;
    GRID_ESTIMATED_TEXT_LINE_HEIGHT
        .mul_add(line_count, GRID_COLOR_SWATCH_HEIGHT + GRID_COLOR_SWATCH_GAP)
}

pub(super) fn file_display_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .filter(|name| !name.is_empty())
        .map_or_else(|| path.to_string(), ToString::to_string)
}

pub(super) fn file_preview_content(record_content: &str) -> String {
    deserialize_file_paths(record_content).join("\n")
}

fn estimated_grid_file_detail_lines(record_content: &str) -> usize {
    let files = deserialize_file_paths(record_content);
    let detail = if files.len() <= 1 {
        files.first().cloned().unwrap_or_default()
    } else {
        files
            .iter()
            .take(1)
            .map(|path| file_display_name(path))
            .collect::<Vec<_>>()
            .join(", ")
    };

    estimated_wrapped_line_count(
        &truncate(
            &detail,
            GRID_CONTENT_PREVIEW_LIMIT,
            TruncateOptions::default(),
        ),
        GRID_CONTENT_PREVIEW_MAX_LINES.saturating_sub(1),
    )
}

pub(super) fn estimated_grid_card_height(record: &ClipboardRecord) -> f32 {
    let body_height = match record.content_type {
        ContentType::Text | ContentType::RichText => {
            if parse_clipboard_color(&record.content).is_some() {
                estimated_grid_color_body_height(&record.content)
            } else {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "line count is bounded by GRID_CONTENT_PREVIEW_MAX_LINES (4)"
                )]
                let lines = estimated_grid_text_lines(&record.content) as f32;
                lines * GRID_ESTIMATED_TEXT_LINE_HEIGHT
            }
        }
        ContentType::Image => GRID_IMAGE_MAX_HEIGHT,
        ContentType::FilePath => {
            #[expect(
                clippy::cast_precision_loss,
                reason = "detail line count is bounded by GRID_CONTENT_PREVIEW_MAX_LINES - 1"
            )]
            let detail_lines = estimated_grid_file_detail_lines(&record.content) as f32;
            GRID_ESTIMATED_FILE_DETAIL_LINE_HEIGHT
                .mul_add(detail_lines, GRID_ESTIMATED_FILE_TITLE_HEIGHT)
        }
    };

    (GRID_ESTIMATED_CARD_CHROME_HEIGHT + body_height)
        .clamp(GRID_CARD_MIN_HEIGHT, GRID_CARD_MAX_HEIGHT)
}

#[cfg(test)]
#[expect(clippy::panic)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::{
        GRID_CARD_MAX_HEIGHT, GRID_CARD_MIN_HEIGHT, GRID_COLOR_SWATCH_GAP,
        GRID_COLOR_SWATCH_HEIGHT, GRID_CONTENT_PREVIEW_MAX_LINES, GRID_ESTIMATED_TEXT_LINE_HEIGHT,
        TruncateOptions, estimated_grid_card_height, estimated_grid_color_body_height,
        estimated_grid_text_lines, file_display_name, file_preview_content,
        list_row_for_selected_index, truncate, visible_list_len,
    };
    use crate::{
        config::LayoutMode,
        repository::{ClipboardRecord, models::ContentType},
    };

    fn test_record(content: &str, content_type: ContentType) -> ClipboardRecord {
        ClipboardRecord {
            id: 1,
            content: content.to_string(),
            content_type,
            pinned: false,
            created_at: Local
                .with_ymd_and_hms(2026, 4, 18, 12, 0, 0)
                .single()
                .unwrap_or_else(|| panic!("invalid test datetime")),
            rich_text_meta: None,
        }
    }

    #[test]
    fn test_truncate_without_max_lines_preserves_short_text() {
        assert_eq!(truncate("abc", 5, TruncateOptions::default()), "abc");
    }

    #[test]
    fn test_truncate_without_max_lines_appends_ellipsis_for_long_text() {
        assert_eq!(truncate("abcdef", 3, TruncateOptions::default()), "abc...");
    }

    #[test]
    fn test_truncate_with_grid_max_lines_allows_up_to_four_lines() {
        let content = "1\n2\n3\n4";
        let options = TruncateOptions::with_max_lines(GRID_CONTENT_PREVIEW_MAX_LINES);

        assert_eq!(truncate(content, 120, options), content);
    }

    #[test]
    fn test_truncate_with_grid_max_lines_truncates_after_four_lines() {
        let content = "1\n2\n3\n4\n5";
        let options = TruncateOptions::with_max_lines(GRID_CONTENT_PREVIEW_MAX_LINES);

        assert_eq!(truncate(content, 120, options), "1\n2\n3\n4...");
    }

    #[test]
    fn test_estimated_grid_text_lines_clamps_wrapped_content() {
        let content = "This is a long line that should wrap more than once in the grid card";

        assert!((2..=GRID_CONTENT_PREVIEW_MAX_LINES).contains(&estimated_grid_text_lines(content)));
    }

    #[test]
    fn test_estimated_grid_card_height_clamps_long_text_cards() {
        let record = test_record(
            "This is a very long line that should wrap repeatedly inside the grid card and reach the maximum height quickly.",
            ContentType::Text,
        );

        let height = estimated_grid_card_height(&record);

        assert!((GRID_CARD_MIN_HEIGHT..=GRID_CARD_MAX_HEIGHT).contains(&height));
    }

    #[test]
    fn test_estimated_grid_color_body_height_adds_large_swatch_space() {
        let content = "#A1B2C3";
        #[expect(
            clippy::cast_precision_loss,
            reason = "line count is bounded by GRID_CONTENT_PREVIEW_MAX_LINES (4)"
        )]
        let lines = estimated_grid_text_lines(content) as f32;
        let expected = GRID_ESTIMATED_TEXT_LINE_HEIGHT
            .mul_add(lines, GRID_COLOR_SWATCH_HEIGHT + GRID_COLOR_SWATCH_GAP);

        assert!((estimated_grid_color_body_height(content) - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_estimated_grid_card_height_when_color_record_uses_taller_layout() {
        let color_record = test_record("#A1B2C3", ContentType::Text);
        let plain_record = test_record("hello", ContentType::Text);

        assert!(
            estimated_grid_card_height(&color_record) > estimated_grid_card_height(&plain_record)
        );
    }

    #[test]
    fn test_file_display_name_when_path_has_filename_returns_filename() {
        assert_eq!(file_display_name("/tmp/archive.zip"), "archive.zip");
    }

    #[test]
    fn test_file_preview_content_when_json_array_returns_joined_paths() {
        let preview = file_preview_content("[\"/tmp/a.txt\",\"/tmp/b.txt\"]");

        assert_eq!(preview, "/tmp/a.txt\n/tmp/b.txt");
    }

    #[test]
    fn test_file_preview_content_when_legacy_string_returns_single_path() {
        let preview = file_preview_content("/tmp/a.txt");

        assert_eq!(preview, "/tmp/a.txt");
    }

    #[test]
    fn test_visible_list_len_uses_record_count_for_list_mode() {
        assert_eq!(visible_list_len(0, LayoutMode::List), 0);
        assert_eq!(visible_list_len(3, LayoutMode::List), 3);
    }

    #[test]
    fn test_visible_list_len_rounds_up_rows_for_grid_mode() {
        assert_eq!(visible_list_len(0, LayoutMode::Grid), 0);
        assert_eq!(visible_list_len(1, LayoutMode::Grid), 1);
        assert_eq!(visible_list_len(2, LayoutMode::Grid), 1);
        assert_eq!(visible_list_len(3, LayoutMode::Grid), 2);
        assert_eq!(visible_list_len(5, LayoutMode::Grid), 3);
    }

    #[test]
    fn test_list_row_for_selected_index_maps_grid_selection_to_row() {
        assert_eq!(list_row_for_selected_index(0, LayoutMode::List), 0);
        assert_eq!(list_row_for_selected_index(3, LayoutMode::List), 3);
        assert_eq!(list_row_for_selected_index(0, LayoutMode::Grid), 0);
        assert_eq!(list_row_for_selected_index(1, LayoutMode::Grid), 0);
        assert_eq!(list_row_for_selected_index(2, LayoutMode::Grid), 1);
        assert_eq!(list_row_for_selected_index(5, LayoutMode::Grid), 2);
    }
}
