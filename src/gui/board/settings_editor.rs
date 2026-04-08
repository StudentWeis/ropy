use gpui::{AppContext, Context, Entity, SharedString, Window};
use gpui_component::{
    IndexPath,
    input::InputState,
    select::{SelectEvent, SelectState},
    slider::{SliderEvent, SliderState},
};

use super::RopyBoard;
use crate::{gui::theme::ThemeId, i18n::Language};

#[allow(clippy::redundant_pub_crate)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct SettingsEditor {
    pub(crate) hotkey_recording: bool,
    pub(crate) pending_hotkey: String,
    pub(crate) hotkey_before_recording: String,
    pub(crate) settings_activation_key_input: Entity<InputState>,
    pub(crate) settings_max_history_input: Entity<InputState>,
    pub(crate) settings_max_storage_input: Entity<InputState>,
    pub(crate) settings_window_opacity_slider: Entity<SliderState>,
    pub(crate) settings_window_opacity_slider_visible: bool,
    pub(crate) selected_theme: usize,
    pub(crate) theme_select: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) window_opacity_percent: u8,
    pub(crate) autostart_enabled: bool,
    pub(crate) selected_language: usize,
    pub(crate) language_select: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) auto_check_enabled: bool,
    pub(crate) include_prerelease_enabled: bool,
    pub(crate) hover_preview_enabled: bool,
}

impl SettingsEditor {
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::fn_params_excessive_bools)]
    #[allow(clippy::missing_const_for_fn)]
    pub(super) fn new(
        pending_hotkey: String,
        hotkey_before_recording: String,
        settings_activation_key_input: Entity<InputState>,
        settings_max_history_input: Entity<InputState>,
        settings_max_storage_input: Entity<InputState>,
        settings_window_opacity_slider: Entity<SliderState>,
        selected_theme: usize,
        theme_select: Entity<SelectState<Vec<SharedString>>>,
        window_opacity_percent: u8,
        selected_language: usize,
        language_select: Entity<SelectState<Vec<SharedString>>>,
        autostart_enabled: bool,
        auto_check_enabled: bool,
        include_prerelease_enabled: bool,
        hover_preview_enabled: bool,
    ) -> Self {
        Self {
            hotkey_recording: false,
            pending_hotkey,
            hotkey_before_recording,
            settings_activation_key_input,
            settings_max_history_input,
            settings_max_storage_input,
            settings_window_opacity_slider,
            settings_window_opacity_slider_visible: false,
            selected_theme,
            theme_select,
            window_opacity_percent,
            autostart_enabled,
            selected_language,
            language_select,
            auto_check_enabled,
            include_prerelease_enabled,
            hover_preview_enabled,
        }
    }
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) struct UpdateManager {
    pub(crate) status: crate::updater::models::UpdateStatus,
}

impl UpdateManager {
    pub(super) const fn new() -> Self {
        Self {
            status: crate::updater::models::UpdateStatus::Idle,
        }
    }
}

/// Build the window opacity slider and subscribe to its change events.
pub(super) fn build_window_opacity_slider(
    opacity_percent: u8,
    window: &Window,
    cx: &mut Context<RopyBoard>,
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
            let SliderEvent::Change(value) = event;
            let opacity_percent = value.start().round() as u8;
            this.settings_editor.window_opacity_percent = opacity_percent;
            let theme = ThemeId::all()
                .get(this.settings_editor.selected_theme)
                .cloned()
                .unwrap_or_default();
            crate::gui::app::set_app_theme(window, cx, &theme, opacity_percent);
            crate::gui::app::apply_window_opacity(window, opacity_percent);
            cx.notify();
        },
    )
    .detach();

    slider
}

/// Build the theme select dropdown and subscribe to its change events.
pub(super) fn build_theme_select(
    selected_theme: usize,
    window: &mut Window,
    cx: &mut Context<RopyBoard>,
) -> Entity<SelectState<Vec<SharedString>>> {
    let theme_items: Vec<SharedString> = ThemeId::all()
        .iter()
        .map(|theme_id| SharedString::from(theme_id.display_name()))
        .collect();
    let theme_select = cx.new(|cx| {
        SelectState::new(
            theme_items,
            Some(IndexPath::default().row(selected_theme)),
            window,
            cx,
        )
    });
    cx.subscribe_in(
        &theme_select,
        window,
        |this, _entity, event: &SelectEvent<Vec<SharedString>>, window, cx| {
            if let SelectEvent::Confirm(Some(val)) = event {
                let themes = ThemeId::all();
                if let Some(idx) = themes
                    .iter()
                    .position(|theme| theme.display_name() == val.as_ref())
                {
                    this.settings_editor.selected_theme = idx;
                    this.save_selected_theme(window, cx);
                }
            }
        },
    )
    .detach();

    theme_select
}

/// Build the language select dropdown and subscribe to its change events.
pub(super) fn build_language_select(
    selected_language: usize,
    window: &mut Window,
    cx: &mut Context<RopyBoard>,
) -> Entity<SelectState<Vec<SharedString>>> {
    let language_items: Vec<SharedString> = Language::all()
        .iter()
        .map(|l| SharedString::from(l.display_name()))
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
                let langs = Language::all();
                if let Some(idx) = langs.iter().position(|l| l.display_name() == val.as_ref()) {
                    this.settings_editor.selected_language = idx;
                    this.save_selected_language(window, cx);
                }
            }
        },
    )
    .detach();

    language_select
}
