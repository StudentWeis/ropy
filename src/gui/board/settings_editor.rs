use gpui_kit::{
    AppContext, Context, Entity, SharedString, Window,
    component::{
        IndexPath,
        input::InputState,
        select::{SelectEvent, SelectState},
        slider::{SliderEvent, SliderState},
    },
};

use super::RopyBoard;
use crate::config::LayoutMode;

#[expect(clippy::redundant_pub_crate)]
pub(crate) mod theme;

#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) struct HotkeyEditorState {
    pub(crate) recording: bool,
    pub(crate) saving: bool,
}

#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) struct SettingsPanelState {
    pub(crate) window_opacity_slider_visible: bool,
    pub(crate) hover_preview_enabled: bool,
    pub(crate) space_preview_enabled: bool,
}

#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) struct AutostartSettingsState {
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) struct UpdateSettingsState {
    pub(crate) auto_check_enabled: bool,
    pub(crate) include_prerelease_enabled: bool,
}

#[expect(clippy::redundant_pub_crate)]
pub(crate) struct SettingsEditor {
    pub(crate) hotkey: HotkeyEditorState,
    pub(crate) pending_hotkey: String,
    pub(crate) hotkey_before_recording: String,
    pub(crate) settings_activation_key_input: Entity<InputState>,
    pub(crate) settings_max_history_input: Entity<InputState>,
    pub(crate) settings_max_storage_input: Entity<InputState>,
    pub(crate) settings_window_opacity_slider: Entity<SliderState>,
    pub(crate) panel_state: SettingsPanelState,
    pub(crate) theme: theme::ThemeEditor,
    pub(crate) selected_layout: usize,
    pub(crate) layout_select: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) window_opacity_percent: u8,
    pub(crate) autostart: AutostartSettingsState,
    pub(crate) selected_language: usize,
    pub(crate) language_select: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) update_settings: UpdateSettingsState,
}

pub(super) fn build_window_opacity_slider(
    opacity_percent: u8,
    window: &Window,
    cx: &mut Context<'_, RopyBoard>,
) -> Entity<SliderState> {
    let slider = cx.new(|_| {
        SliderState::new()
            .min(f32::from(
                crate::config::WindowSettings::MIN_OPACITY_PERCENT,
            ))
            .max(f32::from(
                crate::config::WindowSettings::MAX_OPACITY_PERCENT,
            ))
            .step(1.0)
            .default_value(f32::from(opacity_percent))
    });

    cx.subscribe_in(
        &slider,
        window,
        |this, _, event: &SliderEvent, window, cx| {
            let SliderEvent::Change(value) = event else {
                return;
            };
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "slider value is bounded by MIN/MAX_OPACITY_PERCENT (0..=100)"
            )]
            let opacity_percent = value.start().round() as u8;
            this.settings_editor.window_opacity_percent = opacity_percent;
            crate::gui::app::apply_current_theme(window, cx, opacity_percent);
            crate::gui::app::apply_window_opacity(window, opacity_percent);
            cx.notify();
        },
    )
    .detach();

    slider
}

fn layout_mode_items() -> Vec<SharedString> {
    LayoutMode::all()
        .iter()
        .map(|mode| crate::gui::settings::layout_label(*mode))
        .collect()
}

pub(super) fn sync_layout_select_items(
    layout_select: &Entity<SelectState<Vec<SharedString>>>,
    selected_layout: usize,
    window: &mut Window,
    cx: &mut Context<'_, RopyBoard>,
) {
    let items = layout_mode_items();
    layout_select.update(cx, |state, cx| {
        state.set_items(items.clone(), window, cx);
        state.set_selected_index(Some(IndexPath::default().row(selected_layout)), window, cx);
    });
}

pub(super) fn build_layout_select(
    selected_layout: usize,
    window: &mut Window,
    cx: &mut Context<'_, RopyBoard>,
) -> Entity<SelectState<Vec<SharedString>>> {
    let layout_select = cx.new(|cx| {
        SelectState::new(
            layout_mode_items(),
            Some(IndexPath::default().row(selected_layout)),
            window,
            cx,
        )
    });
    cx.subscribe_in(
        &layout_select,
        window,
        |this, _entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
            if let SelectEvent::Confirm(Some(val)) = event {
                let options = layout_mode_items();
                if let Some(idx) = options
                    .iter()
                    .position(|item| item.as_ref() == val.as_ref())
                {
                    this.settings_editor.selected_layout = idx;
                    this.save_selected_layout(window, cx);
                }
            }
        },
    )
    .detach();

    layout_select
}

pub(super) fn build_language_select(
    selected_language: usize,
    window: &mut Window,
    cx: &mut Context<'_, RopyBoard>,
) -> Entity<SelectState<Vec<SharedString>>> {
    let language_items: Vec<SharedString> = crate::gui::settings::available_languages()
        .iter()
        .map(crate::gui::settings::language_display_name)
        .collect();
    let language_select = cx.new(|cx| {
        SelectState::new(
            language_items,
            Some(IndexPath::default().row(selected_language)),
            window,
            cx,
        )
    });
    cx.subscribe_in(
        &language_select,
        window,
        |this, _entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
            if let SelectEvent::Confirm(Some(val)) = event {
                let langs = crate::gui::settings::available_languages();
                if let Some(idx) = langs
                    .iter()
                    .position(|l| crate::gui::settings::language_display_name(l) == val.as_ref())
                {
                    this.settings_editor.selected_language = idx;
                    this.save_selected_language(window, cx);
                }
            }
        },
    )
    .detach();

    language_select
}

impl SettingsEditor {
    pub(super) fn new(window: &mut Window, cx: &mut Context<'_, RopyBoard>) -> Self {
        let settings = crate::gui::settings::GlobalSettings::read(cx, Clone::clone);
        let activation_key = &settings.hotkey.activation_key;
        let layout_mode = settings.layout.mode;
        let activation_key_placeholder = RopyBoard::hotkey_placeholder_text(activation_key);
        let settings_activation_key_input = cx
            .new(|cx| InputState::new(window, cx).placeholder(activation_key_placeholder.clone()));
        let settings_max_history_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(settings.storage.max_history_records.to_string())
        });
        let settings_max_storage_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(settings.storage.max_storage_records.to_string())
        });
        let settings_window_opacity_slider =
            build_window_opacity_slider(settings.window.opacity_percent, window, cx);

        let theme = theme::ThemeEditor::new(window, cx);

        let selected_layout = LayoutMode::all()
            .iter()
            .position(|mode| mode == &layout_mode)
            .unwrap_or_default();
        let layout_select = build_layout_select(selected_layout, window, cx);

        let selected_language = crate::gui::settings::available_languages()
            .iter()
            .position(|lang| lang == &settings.language)
            .unwrap_or(0);
        let language_select = build_language_select(selected_language, window, cx);

        Self {
            hotkey: HotkeyEditorState::default(),
            pending_hotkey: activation_key.clone(),
            hotkey_before_recording: activation_key.clone(),
            settings_activation_key_input,
            settings_max_history_input,
            settings_max_storage_input,
            settings_window_opacity_slider,
            theme,
            selected_layout,
            layout_select,
            window_opacity_percent: settings.window.opacity_percent,
            selected_language,
            language_select,
            autostart: AutostartSettingsState {
                enabled: settings.autostart.enabled,
            },
            update_settings: UpdateSettingsState {
                auto_check_enabled: settings.update.auto_check,
                include_prerelease_enabled: settings.update.include_prerelease,
            },
            panel_state: SettingsPanelState {
                window_opacity_slider_visible: false,
                hover_preview_enabled: settings.preview.hover_preview_enabled,
                space_preview_enabled: settings.preview.space_preview_enabled,
            },
        }
    }
}
