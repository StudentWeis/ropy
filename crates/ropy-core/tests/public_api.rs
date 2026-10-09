//! Verify persistence and recovery through the API available to desktop consumers.

use ropy_core::{
    config::{Language, Settings, ThemeId},
    repository::{ClipboardRepository, assets::load_rich_text_html},
};

#[test]
fn test_repository_reopen_preserves_rich_text_pin_and_favorite()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("history.redb");
    let images = directory.path().join("images");
    let id = {
        let repository = ClipboardRepository::open(&database, images.clone())?;
        let record = repository.save_rich_text("hello".into(), Some("<b>hello</b>"), None)?;
        repository.toggle_pin(record.id)?;
        repository.toggle_favorite(record.id)?;
        repository.flush()?;
        record.id
    };

    let repository = ClipboardRepository::open(&database, images)?;
    let records = repository.get_display_records(1)?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, id);
    assert!(records[0].pinned);
    assert_eq!(repository.favorite_ids()?, vec![id]);
    assert_eq!(
        records[0]
            .rich_text_meta
            .as_ref()
            .and_then(load_rich_text_html)
            .as_deref(),
        Some("<b>hello</b>")
    );
    Ok(())
}

#[test]
fn test_settings_explicit_path_preserves_recovery_and_serialized_identifiers()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("config.toml");
    let invalid = "[storage\nmax_history_records = broken";
    std::fs::write(&path, invalid)?;
    assert!(Settings::load_from_dir(directory.path()).is_err());
    assert!(Settings::recovery_defaults().save_to_file(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path)?, invalid);

    std::fs::write(&path, "theme = 'Light'\nlanguage = 'zh-CN'\n")?;
    let loaded = Settings::load_from_dir(directory.path())?;
    assert_eq!(loaded.theme, ThemeId::new("ropy-light"));
    assert_eq!(loaded.language, Language::new("zh-CN"));
    loaded.save_to_file(&path)?;
    assert_eq!(
        Settings::load_from_dir(directory.path())?.theme,
        loaded.theme
    );
    Ok(())
}
