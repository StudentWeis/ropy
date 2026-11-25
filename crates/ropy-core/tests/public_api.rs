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
fn test_image_payloads_remain_owned_after_database_reopen() -> Result<(), Box<dyn std::error::Error>>
{
    use ropy_core::repository::assets::{PendingImage, thumb_path_for};
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let images = root.join("images");
    let a = root.join("a.redb");
    let b = root.join("b.redb");
    let pixels = image::DynamicImage::new_rgba8(2, 3);
    let first = ClipboardRepository::open(&a, images.clone())?;
    let record = first.save_pending_image(PendingImage::encode(&pixels, 42, &images)?)?;
    let path = std::path::Path::new(&record.content);
    let thumb = thumb_path_for(path);
    drop(first);
    assert!(ClipboardRepository::open(&b, images.clone()).is_err());
    let second = ClipboardRepository::open(&b, root.join("other-images"))?;
    let neighbor =
        second.save_pending_image(PendingImage::encode(&pixels, 42, second.images_dir())?)?;
    second.clear()?;
    assert!(!std::path::Path::new(&neighbor.content).exists());
    assert_eq!(image::open(path)?.to_rgba8(), pixels.to_rgba8());
    assert!(thumb.is_file());
    let first = ClipboardRepository::open(&a, images)?;
    assert!(first.delete(record.id)?);
    assert!(!path.exists());
    assert!(!thumb.exists());
    Ok(())
}

#[test]
fn test_repository_neighbors_preserve_each_others_payloads()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    // Reopen with a custom image directory to exercise existing records whose
    // metadata points into the original shared rich-text directory.
    let neighbor_db = root.join("two.redb");
    let original = ClipboardRepository::open(&neighbor_db, root.join("images"))?;
    let old = original.save_rich_text("legacy".into(), Some("<i>legacy</i>"), None)?;
    let old_meta = old.rich_text_meta.as_ref().ok_or("missing legacy HTML")?;
    drop(original);
    // Model a database written before image-directory ownership was recorded.
    std::fs::remove_file(root.join("images/.ropy-database"))?;
    let first = ClipboardRepository::open(&root.join("one.redb"), root.join("images"))?;
    let a = first.save_rich_text("a".into(), Some("<b>a</b>"), None)?;
    let a_meta = a.rich_text_meta.as_ref().ok_or("missing first HTML")?;
    let second = ClipboardRepository::open(&neighbor_db, root.join("images-two"))?;
    assert_eq!(load_rich_text_html(a_meta).as_deref(), Some("<b>a</b>"));
    let b = second.save_rich_text("b".into(), Some("<b>b</b>"), None)?;
    let b_meta = b.rich_text_meta.as_ref().ok_or("missing second HTML")?;
    assert_ne!(
        std::path::Path::new(a_meta.html_path.as_deref().ok_or("missing first path")?).parent(),
        std::path::Path::new(b_meta.html_path.as_deref().ok_or("missing second path")?).parent(),
    );
    first.clear()?;
    assert!(load_rich_text_html(a_meta).is_none());
    assert_eq!(load_rich_text_html(b_meta).as_deref(), Some("<b>b</b>"));
    assert_eq!(
        load_rich_text_html(old_meta).as_deref(),
        Some("<i>legacy</i>")
    );
    second.clear()?;
    assert!(load_rich_text_html(b_meta).is_none());
    assert!(load_rich_text_html(old_meta).is_none());
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
