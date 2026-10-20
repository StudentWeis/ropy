//! Search contracts shared by the desktop and the performance suite.

use std::collections::HashSet;

use ropy_core::{
    repository::{ClipboardRecord, ContentType},
    search::{ContentFilter, SearchOptions, filter_records_by_query},
};

#[rstest::rstest]
#[case("İSTANBUL", "i\u{307}stanbul", false, false, true)]
#[case("ΟΣ", "ος", false, true, true)]
#[case("ΟΣ", "οσ", false, false, false)]
#[case("Straße", "STRASSE", false, false, false)]
#[case("你好世界", "世界", false, false, true)]
#[case("你好世界", "世界", false, true, false)]
#[case("你好 世界！", "世界", false, true, true)]
#[case("xé-é-é", "é-é", false, true, true)]
#[case("xa-a-a", "a-a", true, true, true)]
#[case("cat_cat cat2", "cat", false, true, false)]
#[case("éCat", "cat", false, true, false)]
#[case("🙂Cat🙂", "cat", false, true, true)]
#[case("e\u{301}", "é", false, false, false)]
#[case("ANY", "", true, true, true)]
fn test_search_unicode_and_boundaries_preserve_literal_contract(
    #[case] content: &str,
    #[case] query: &str,
    #[case] case_sensitive: bool,
    #[case] whole_word: bool,
    #[case] expected: bool,
) {
    let records = [record(1, content, ContentType::Text)];
    let mut options = SearchOptions::default();
    options.case_sensitive = case_sensitive;
    options.whole_word = whole_word;
    let matches = filter_records_by_query(
        &records,
        query,
        ContentFilter::All,
        options,
        &HashSet::new(),
        false,
    );
    assert_eq!(matches, if expected { vec![0] } else { vec![] });
}

#[rstest::rstest]
#[case(r#"["file:///tmp/Final%20Report.pdf"]"#, "final report", true)]
#[case(r#"["/tmp/报告.pdf","/tmp/notes.txt"]"#, "报告", true)]
#[case(r#"["/tmp/\u62a5\u544a.pdf"]"#, "报告", true)]
#[case(r#"["/tmp/report%20final.txt"]"#, "report final", false)]
#[case(r#"["/tmp/a","/tmp/b"]"#, "a/tmp", false)]
#[case("file://localhost/tmp/Report%20Final.pdf", "report final", true)]
#[case("[broken", "broken", true)]
#[case(r#"["", "   "]"#, " ", false)]
fn test_search_file_paths_use_decoded_individual_paths(
    #[case] content: &str,
    #[case] query: &str,
    #[case] expected: bool,
) {
    let records = [record(1, content, ContentType::FilePath)];
    let matches = filter_records_by_query(
        &records,
        query,
        ContentFilter::Files,
        SearchOptions::default(),
        &HashSet::new(),
        false,
    );
    assert_eq!(matches, if expected { vec![0] } else { vec![] });
}

#[test]
fn test_search_filters_preserve_source_indices_and_current_record_content() {
    let mut records = vec![
        record(9, "Report", ContentType::Text),
        record(3, "REPORT", ContentType::RichText),
        record(7, "Report", ContentType::Image),
        record(1, "/tmp/report", ContentType::FilePath),
    ];
    let favorites = HashSet::from([3, 7, 1]);
    let search = |records: &[ClipboardRecord], filter| {
        filter_records_by_query(
            records,
            "report",
            filter,
            SearchOptions::default(),
            &favorites,
            true,
        )
    };
    assert_eq!(search(&records, ContentFilter::All), vec![1, 3]);
    assert_eq!(search(&records, ContentFilter::Image), vec![2]);
    assert_eq!(search(&records, ContentFilter::Text), vec![1]);
    records[1].content = "updated".into();
    records.remove(0);
    assert_eq!(search(&records, ContentFilter::All), vec![2]);
}

fn record(id: u64, content: &str, content_type: ContentType) -> ClipboardRecord {
    ClipboardRecord::new(id, content.into(), chrono::Local::now(), content_type)
}
