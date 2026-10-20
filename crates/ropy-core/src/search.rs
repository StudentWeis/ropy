//! Literal clipboard history search, independent of the desktop runtime.

use std::{borrow::Cow, collections::HashSet, hash::BuildHasher};

use memchr::memmem::Finder;

use crate::repository::{ClipboardRecord, ContentType, deserialize_file_paths};

/// Content type filter for the clipboard history view.
///
/// This filter controls which *content types* are shown. It is independent of
/// the favorites toggle (`favorites_only`) so users can combine them – e.g.
/// "show only favorited images".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentFilter {
    /// Show all content types
    #[default]
    All,
    /// Show only text records
    Text,
    /// Show only image records
    Image,
    /// Show only file records
    Files,
}

/// Literal search options. Defaults to case-insensitive substring matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct SearchOptions {
    /// Preserve case instead of applying Rust Unicode lowercase to both strings.
    pub case_sensitive: bool,
    /// Require boundaries outside Unicode alphanumeric characters and underscores.
    pub whole_word: bool,
}

fn is_token_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn has_word_boundaries(content: &str, start: usize, end: usize) -> bool {
    let previous = content[..start].chars().next_back();
    let next = content[end..].chars().next();

    let has_left_boundary = previous.is_none_or(|ch| !is_token_char(ch));
    let has_right_boundary = next.is_none_or(|ch| !is_token_char(ch));

    has_left_boundary && has_right_boundary
}

fn normalized_text(text: &str, case_sensitive: bool) -> Cow<'_, str> {
    if case_sensitive {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.to_lowercase())
    }
}

// The caller normalizes the nonempty query once for the entire record set.
fn text_matches_query(content: &str, query: &Finder<'_>, options: SearchOptions) -> bool {
    let content = normalized_text(content, options.case_sensitive);
    if !options.whole_word {
        return query.find(content.as_bytes()).is_some();
    }

    let mut offset = 0;
    while let Some(relative) = query.find(&content.as_bytes()[offset..]) {
        let start = offset + relative;
        // A nonempty UTF-8 needle can only match at UTF-8 character boundaries.
        if has_word_boundaries(&content, start, start + query.needle().len()) {
            return true;
        }
        // Advance one character, not one match, so overlapping candidates survive.
        offset = content.ceil_char_boundary(start + 1);
    }
    false
}

fn file_paths_match_query(content: &str, query: &Finder<'_>, options: SearchOptions) -> bool {
    deserialize_file_paths(content)
        .iter()
        .any(|path| text_matches_query(path, query, options))
}

const fn is_text_like_record(content_type: &ContentType) -> bool {
    matches!(content_type, ContentType::Text | ContentType::RichText)
}

/// Return matching source indices in input order, without sorting or mutating records.
///
/// Images match only an empty query, except that the image filter ignores the query.
/// File records search decoded, normalized paths individually.
/// Case-insensitive matching uses Unicode lowercase, not full case folding or normalization.
///
/// `filter` and `favorites_only` are independent dimensions:
/// - `filter` restricts by content type (Text / Image / Files / All).
/// - `favorites_only` further restricts to favorited records when `true`.
#[must_use]
pub fn filter_records_by_query<S: BuildHasher>(
    records: &[ClipboardRecord],
    query: &str,
    filter: ContentFilter,
    options: SearchOptions,
    favorite_ids: &HashSet<u64, S>,
    favorites_only: bool,
) -> Vec<usize> {
    // Image-only filtering ignores text; do not lowercase a query it never uses.
    let normalized_query = normalized_text(
        query,
        options.case_sensitive || filter == ContentFilter::Image,
    );
    let finder = (!query.is_empty() && filter != ContentFilter::Image)
        .then(|| Finder::new(normalized_query.as_bytes()));
    records
        .iter()
        .enumerate()
        .filter(|(_, record)| {
            // Apply favorites filter (independent dimension)
            if favorites_only && !favorite_ids.contains(&record.id) {
                return false;
            }

            // Apply content type filter
            let passes_type_filter = match filter {
                ContentFilter::All => true,
                ContentFilter::Text => is_text_like_record(&record.content_type),
                ContentFilter::Image => record.content_type == ContentType::Image,
                ContentFilter::Files => record.content_type == ContentType::FilePath,
            };

            if !passes_type_filter {
                return false;
            }

            // An absent matcher means either an empty query or image-only filtering.
            let Some(query) = &finder else {
                return true;
            };

            match record.content_type {
                ContentType::Text | ContentType::RichText => {
                    text_matches_query(&record.content, query, options)
                }
                ContentType::FilePath => file_paths_match_query(&record.content, query, options),
                ContentType::Image => false,
            }
        })
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests;
