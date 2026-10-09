//! GPUI owns the live settings snapshot; the config module owns its format.

use gpui_kit::{App, BorrowAppContext, Global, ReadGlobal, SharedString};

use crate::{
    config::{LayoutMode, Settings},
    i18n::I18n,
};

#[derive(Debug, Clone)]
pub(crate) struct GlobalSettings(Settings);

impl Global for GlobalSettings {}

impl GlobalSettings {
    pub(crate) const fn new(settings: Settings) -> Self {
        Self(settings)
    }

    pub(crate) fn read<R>(cx: &App, reader: impl FnOnce(&Settings) -> R) -> R {
        reader(&Self::global(cx).0)
    }

    pub(crate) fn update<R>(cx: &mut App, update: impl FnOnce(&mut Settings) -> R) -> R {
        cx.update_global::<Self, _>(|global, _| update(&mut global.0))
    }
}

pub(crate) fn layout_label(mode: LayoutMode, cx: &App) -> SharedString {
    let label = match mode {
        LayoutMode::List => I18n::translate(cx, "settings_layout_list"),
        LayoutMode::Grid => I18n::translate(cx, "settings_layout_grid"),
    };
    SharedString::from(label)
}
