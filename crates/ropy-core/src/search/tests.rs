use super::*;

fn empty_favorites() -> HashSet<u64> {
    HashSet::new()
}

#[rstest::rstest]
#[case("xa-a-a", "a-a", true)]
#[case("xé-é-é", "é-é", true)]
#[case("xa-a-ax", "a-a", false)]
fn test_whole_word_overlapping_matches_check_every_start(
    #[case] content: &str,
    #[case] query: &str,
    #[case] expected: bool,
) {
    assert_eq!(
        text_matches_query(
            content,
            &Finder::new(query),
            SearchOptions {
                case_sensitive: true,
                whole_word: true,
            }
        ),
        expected
    );
}

/// Helper: build a mixed set of test records (2 text + 1 image)
fn mixed_records() -> Vec<ClipboardRecord> {
    vec![
        ClipboardRecord::new(
            1,
            "Hello World".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
        ClipboardRecord::new(
            2,
            "Goodbye World".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
        ClipboardRecord::new(
            3,
            "image_data".to_string(),
            chrono::Local::now(),
            ContentType::Image,
        ),
    ]
}

// --- ContentFilter::All (default) tests ---

#[test]
fn test_filter_all_no_query_returns_everything() {
    let records = mixed_records();
    let result = filter_records_by_query(
        &records,
        "",
        ContentFilter::All,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );
    assert_eq!(result, vec![0, 1, 2]);
}

#[test]
fn test_filter_all_with_query_matches_text_only() {
    let records = mixed_records();
    let result = filter_records_by_query(
        &records,
        "world",
        ContentFilter::All,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );
    assert_eq!(result, vec![0, 1]);
}

#[test]
fn test_filter_all_with_query_matches_rich_text_records() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "Hello World".to_string(),
            chrono::Local::now(),
            ContentType::RichText,
        ),
        ClipboardRecord::new(
            2,
            "image_data".to_string(),
            chrono::Local::now(),
            ContentType::Image,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "world",
        ContentFilter::All,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );

    assert_eq!(result, vec![0]);
}

#[test]
fn test_filter_text_includes_rich_text_records() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "Plain text".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
        ClipboardRecord::new(
            2,
            "Formatted text".to_string(),
            chrono::Local::now(),
            ContentType::RichText,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "text",
        ContentFilter::Text,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );

    assert_eq!(result, vec![0, 1]);
}

#[test]
fn test_search_contains_case_insensitive_matches_all_variants() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "Hello World".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
        ClipboardRecord::new(
            2,
            "HELLO world".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "hello",
        ContentFilter::All,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );
    assert_eq!(result, vec![0, 1]);
}

#[test]
fn test_search_contains_case_sensitive_matches_only_same_case() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "Hello World".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
        ClipboardRecord::new(
            2,
            "hello world".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "Hello",
        ContentFilter::All,
        SearchOptions {
            case_sensitive: true,
            whole_word: false,
        },
        &empty_favorites(),
        false,
    );

    assert_eq!(result, vec![0]);
}

#[test]
fn test_search_whole_word_case_insensitive_matches_token_boundaries() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "Say hello world".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
        ClipboardRecord::new(
            2,
            "say HELLO again".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
        ClipboardRecord::new(
            3,
            "shelloworld".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "hello",
        ContentFilter::All,
        SearchOptions {
            case_sensitive: false,
            whole_word: true,
        },
        &empty_favorites(),
        false,
    );

    assert_eq!(result, vec![0, 1]);
}

#[test]
fn test_search_whole_word_case_sensitive_rejects_case_mismatch() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "say Hello again".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
        ClipboardRecord::new(
            2,
            "say hello again".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "Hello",
        ContentFilter::All,
        SearchOptions {
            case_sensitive: true,
            whole_word: true,
        },
        &empty_favorites(),
        false,
    );

    assert_eq!(result, vec![0]);
}

#[test]
fn test_search_whole_word_partial_token_returns_no_match() {
    let records = vec![ClipboardRecord::new(
        1,
        "hello_world hello2".to_string(),
        chrono::Local::now(),
        ContentType::Text,
    )];

    let result = filter_records_by_query(
        &records,
        "hello",
        ContentFilter::All,
        SearchOptions {
            case_sensitive: false,
            whole_word: true,
        },
        &empty_favorites(),
        false,
    );

    assert_eq!(result, Vec::<usize>::new());
}

#[test]
fn test_filter_all_with_query_no_matches() {
    let records = vec![ClipboardRecord::new(
        1,
        "Hello".to_string(),
        chrono::Local::now(),
        ContentType::Text,
    )];

    let result = filter_records_by_query(
        &records,
        "xyz",
        ContentFilter::All,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );
    assert_eq!(result, Vec::<usize>::new());
}

#[test]
fn test_filter_all_with_query_excludes_image() {
    let records = vec![ClipboardRecord::new(
        1,
        "Image content".to_string(),
        chrono::Local::now(),
        ContentType::Image,
    )];

    let result = filter_records_by_query(
        &records,
        "image",
        ContentFilter::All,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );
    assert_eq!(result, Vec::<usize>::new());
}

#[test]
fn test_filter_all_with_query_matches_file_path_records() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "[\"/tmp/final-report.pdf\",\"/tmp/notes.txt\"]".to_string(),
            chrono::Local::now(),
            ContentType::FilePath,
        ),
        ClipboardRecord::new(
            2,
            "Hello World".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "report",
        ContentFilter::All,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );

    assert_eq!(result, vec![0]);
}

#[test]
fn test_filter_favorites_with_query_matches_file_path_records() {
    let records = vec![ClipboardRecord::new(
        7,
        "/tmp/archive.zip".to_string(),
        chrono::Local::now(),
        ContentType::FilePath,
    )];
    let favorites = HashSet::from([7]);

    let result = filter_records_by_query(
        &records,
        "archive",
        ContentFilter::All,
        SearchOptions::default(),
        &favorites,
        true,
    );

    assert_eq!(result, vec![0]);
}

// --- ContentFilter::Text tests ---

#[test]
fn test_filter_text_no_query_returns_text_only() {
    let records = mixed_records();
    let result = filter_records_by_query(
        &records,
        "",
        ContentFilter::Text,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );
    assert_eq!(result, vec![0, 1]);
}

#[test]
fn test_filter_text_with_query_matches_within_text() {
    let records = mixed_records();
    let result = filter_records_by_query(
        &records,
        "hello",
        ContentFilter::Text,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );
    assert_eq!(result, vec![0]);
}

// --- ContentFilter::Image tests ---

#[test]
fn test_filter_image_no_query_returns_images_only() {
    let records = mixed_records();
    let result = filter_records_by_query(
        &records,
        "",
        ContentFilter::Image,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );
    assert_eq!(result, vec![2]);
}

#[test]
fn test_filter_image_with_query_ignores_query_for_all_search_modes() {
    let records = mixed_records();
    let no_favorites = empty_favorites();
    for options in [
        SearchOptions::default(),
        SearchOptions {
            case_sensitive: false,
            whole_word: true,
        },
    ] {
        let result = filter_records_by_query(
            &records,
            "nonexistent",
            ContentFilter::Image,
            options,
            &no_favorites,
            false,
        );
        assert_eq!(result, vec![2]);
    }
}

#[test]
fn test_filter_files_no_query_returns_files_only() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "[\"/tmp/report.pdf\"]".to_string(),
            chrono::Local::now(),
            ContentType::FilePath,
        ),
        ClipboardRecord::new(
            2,
            "hello".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "",
        ContentFilter::Files,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );

    assert_eq!(result, vec![0]);
}

#[test]
fn test_filter_files_with_query_matches_file_paths_only() {
    let records = vec![
        ClipboardRecord::new(
            1,
            "[\"/tmp/report.pdf\",\"/tmp/notes.txt\"]".to_string(),
            chrono::Local::now(),
            ContentType::FilePath,
        ),
        ClipboardRecord::new(
            2,
            "report body".to_string(),
            chrono::Local::now(),
            ContentType::Text,
        ),
    ];

    let result = filter_records_by_query(
        &records,
        "report",
        ContentFilter::Files,
        SearchOptions::default(),
        &empty_favorites(),
        false,
    );

    assert_eq!(result, vec![0]);
}

// --- ContentFilter::Favorites tests ---

#[test]
fn test_filter_favorites_no_query_returns_favorited_only() {
    let records = mixed_records();
    let favorites: HashSet<u64> = [1, 3].into_iter().collect();
    let result = filter_records_by_query(
        &records,
        "",
        ContentFilter::All,
        SearchOptions::default(),
        &favorites,
        true,
    );
    assert_eq!(result, vec![0, 2]);
}

#[test]
fn test_filter_favorites_with_query_searches_text_favorites() {
    let records = mixed_records();
    let favorites: HashSet<u64> = [1, 2].into_iter().collect();
    let result = filter_records_by_query(
        &records,
        "hello",
        ContentFilter::All,
        SearchOptions::default(),
        &favorites,
        true,
    );
    assert_eq!(result, vec![0]);
}

#[test]
fn test_filter_favorites_empty_set_returns_nothing() {
    let records = mixed_records();
    let result = filter_records_by_query(
        &records,
        "",
        ContentFilter::All,
        SearchOptions::default(),
        &empty_favorites(),
        true,
    );
    assert_eq!(result, Vec::<usize>::new());
}
