#![allow(clippy::unwrap_used)]

use gpui_kit::{ReadGlobal, TestAppContext, test::TestWindowExt};
use rstest::rstest;

use super::*;
use crate::gui::board::ActivePanel;

#[rstest]
#[case(&["--promo"])]
#[case(&["--promo", "missing", "en", "ready"])]
#[case(&["--promo", "ropy-light", "missing", "ready"])]
#[case(&["--promo", "ropy-light", "en", "ready", "extra"])]
#[case(&["--compose-promo", "out.png", "one.png"])]
fn test_promo_arguments_invalid_rejected(#[case] args: &[&str]) {
    let args = args.iter().map(OsString::from).collect::<Vec<_>>();
    assert!(PromoCommand::parse(&args).is_err());
}

#[test]
fn test_promo_arguments_normal_launch_unchanged() {
    assert!(PromoCommand::parse(&[]).unwrap().is_none());
}
#[test]
fn test_demo_records_repeatable_and_localized() {
    let logo = Path::new("demo-logo.png");
    let english = I18n::load_i18n(Language::new("en"));
    let records = demo_records(logo, &english).unwrap();
    assert_eq!(records, demo_records(logo, &english).unwrap());
    assert_eq!(
        records.iter().map(|r| r.id).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(
        records[0]
            .created_at
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        "2026-04-02 13:23:14"
    );
    assert_eq!(records[3].content_type, ContentType::Image);
    let chinese = demo_records(logo, &I18n::load_i18n(Language::new("zh-CN"))).unwrap();
    assert_ne!(records[1].content, chinese[1].content);
}

#[gpui_kit::test]
fn test_promo_input_shield_prevents_settings_and_copy(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(demo_settings(ThemeId::default(), Language::new("en")));
        cx.set_global(I18n::default());
        cx.set_global(GlobalRepository::new(None));
    });
    let records = demo_records(Path::new("missing-logo.png"), &I18n::default()).unwrap();
    let (handle, view) = cx.update(|cx| open_promo_window(records, cx).unwrap());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-button", cx);
        window.press("tab", cx);
        window.press("enter", cx);
        window.input("/", cx);
        window.render_frame(cx);
        let board = view.read(cx).board.read(cx);
        assert_eq!(board.active_panel, ActivePanel::ClipboardList);
        assert_eq!(board.selected_index, 0);
        assert!(board.search_input.read(cx).value().is_empty());
        assert!(board.copy_tx.is_closed());
        assert!(GlobalRepository::global(cx).cloned().is_none());
        assert_eq!(board.layout_mode, LayoutMode::List);
        assert!(board.pinned);
    })
    .unwrap();
}

#[test]
fn test_compose_transparent_canvas_decorates_each_screenshot() {
    let temp = tempfile::tempdir().unwrap();
    let inputs = std::array::from_fn(|ix| temp.path().join(format!("{ix}.png")));
    for (path, value) in inputs.iter().zip([40, 80, 120, 160]) {
        let mut image = RgbaImage::from_pixel(40, 50, Rgba([value, 0, 0, 255]));
        image.put_pixel(1, 1, Rgba([0, value, 0, 255]));
        // A transparent corner must not acquire a rectangular frame.
        for y in 0..8 {
            for x in 0..8 {
                image.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
        image.save(path).unwrap();
    }
    let output = temp.path().join("output.png");
    compose(&output, &inputs).unwrap();
    let result = image::open(output).unwrap().into_rgba8();
    assert_eq!(result.dimensions(), (180, 200));
    for (path, (x, y)) in inputs
        .iter()
        .zip([(28, 28), (112, 28), (28, 122), (112, 122)])
    {
        let original = image::open(path).unwrap().into_rgba8();
        for (px, py, pixel) in original.enumerate_pixels() {
            if pixel[3] == 255 {
                assert_eq!(result.get_pixel(x + px, y + py), pixel);
            }
        }
        let border = result.get_pixel(x - 1, y + 25);
        assert!(border[3] >= 180);
        assert!(border[0] > 80 && border[2] > border[0]);
        let shadow = result.get_pixel(x + 20, y + 54);
        assert_eq!(&shadow.0[..3], &[0, 0, 0]);
        assert!(shadow[3] > 0 && shadow[3] < 80);
        assert!(result.get_pixel(x, y)[3] < 180);
    }
    for (x, y) in [(0, 0), (90, 0), (179, 199)] {
        assert_eq!(result.get_pixel(x, y)[3], 0);
    }
}

#[test]
fn test_compose_mismatched_or_blank_images_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let inputs = std::array::from_fn(|ix| temp.path().join(format!("{ix}.png")));
    for path in &inputs {
        RgbaImage::new(2, 2).save(path).unwrap();
    }
    let output = temp.path().join("output.png");
    assert!(matches!(
        compose(&output, &inputs),
        Err(PromoError::InvalidScreenshots)
    ));
    for (path, width) in inputs.iter().zip([3, 2, 2, 2]) {
        let mut image = RgbaImage::new(width, 3);
        image.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        image.save(path).unwrap();
    }
    assert!(matches!(
        compose(&output, &inputs),
        Err(PromoError::InvalidScreenshots)
    ));
    assert!(!output.exists());
}

#[rstest]
#[case("ropy-light", "en")]
#[case("ropy-dark", "zh-CN")]
#[case("nord-light", "ja")]
#[case("everforest-night", "en")]
fn test_promo_arguments_valid_capture_accepted(#[case] theme: &str, #[case] language: &str) {
    let args = ["--promo", theme, language, "ready"].map(OsString::from);
    let Some(PromoCommand::Capture {
        theme: actual,
        language: locale,
        ready,
    }) = PromoCommand::parse(&args).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(actual.code(), theme);
    assert_eq!(locale.code(), language);
    assert_eq!(ready, Path::new("ready"));
}

#[gpui_kit::test]
fn test_promo_readiness_after_image_load_and_render_writes_child_pid(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let logo = temp.path().join("logo.png");
    std::fs::write(&logo, Assets::get("logo.png").unwrap().data).unwrap();
    let ready = temp.path().join("ready");
    let failure = CaptureFailure::default();
    let window = cx.add_empty_window();
    window.update(|window, _| {
        signal_when_rendered(
            window,
            Resource::Path(logo.into()),
            ready.clone(),
            failure.clone(),
        );
    });
    assert!(!ready.exists());
    for _ in 0..10 {
        window.run_until_parked();
        window.update(|window, cx| {
            window.render_frame(cx);
            window.simulate_next_frame(cx);
        });
        if ready.exists() {
            break;
        }
    }
    assert!(failure.borrow().is_none());
    assert_eq!(
        std::fs::read_to_string(ready).unwrap(),
        std::process::id().to_string()
    );
}
