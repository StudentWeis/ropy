//! Theme selectors keep stable theme IDs; settings remain the source of truth.

use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, SharedString, Styled,
    Window,
    component::{
        IndexPath, h_flex,
        select::{SelectDelegate, SelectEvent, SelectItem, SelectState},
        separator::Separator,
    },
    div, rgb,
};

use super::RopyBoard;
use crate::{
    config::Settings,
    gui::{
        settings::GlobalSettings,
        theme::{ThemeCatalog, ThemeId, ThemeMode},
    },
};

#[derive(Clone, Copy)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) enum ThemeTarget {
    Fixed,
    Light,
    Dark,
}

impl ThemeTarget {
    const fn mode(self) -> Option<ThemeMode> {
        match self {
            Self::Fixed => None,
            Self::Light => Some(ThemeMode::Light),
            Self::Dark => Some(ThemeMode::Dark),
        }
    }

    fn selected(self, settings: &Settings, catalog: &ThemeCatalog, mode: ThemeMode) -> ThemeId {
        match self {
            Self::Fixed => catalog.get(&settings.theme).map_or_else(
                || catalog.for_mode(&settings.theme, mode),
                |theme| theme.id().clone(),
            ),
            Self::Light => catalog.for_mode(&settings.appearance.light_theme, ThemeMode::Light),
            Self::Dark => catalog.for_mode(&settings.appearance.dark_theme, ThemeMode::Dark),
        }
    }

    pub(crate) fn set(self, settings: &mut Settings, id: ThemeId) {
        match self {
            Self::Fixed => settings.theme = id,
            Self::Light => settings.appearance.light_theme = id,
            Self::Dark => settings.appearance.dark_theme = id,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) enum ThemeChoice {
    Theme(ThemeId),
    Customize,
}

#[derive(Clone)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) struct ThemeOption {
    id: ThemeChoice,
    title: SharedString,
    swatch: [u32; 3],
}

impl ThemeOption {
    fn content(&self) -> AnyElement {
        if self.id == ThemeChoice::Customize {
            // Keep the separator out of the row's line box so it cannot push
            // the command label below the other menu items' vertical center.
            return h_flex()
                .w_full()
                .relative()
                .justify_center()
                // Balance the Select row's trailing checkmark gutter.
                .pl_4()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .child(Separator::horizontal()),
                )
                .child(self.title.clone())
                .into_any_element();
        }
        // These are palette samples (data colors), not colors of the surrounding control.
        h_flex()
            .gap_2()
            .min_w_0()
            .child(
                h_flex()
                    .size_4()
                    .flex_shrink_0()
                    .rounded_full()
                    .overflow_hidden()
                    .children(
                        self.swatch
                            .map(|color| div().h_full().flex_1().bg(rgb(color))),
                    ),
            )
            .child(div().min_w_0().text_ellipsis().child(self.title.clone()))
            .into_any_element()
    }
}

impl SelectItem for ThemeOption {
    type Value = ThemeChoice;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &ThemeChoice {
        &self.id
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(self.content().into_any_element())
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.content()
    }
}

#[expect(clippy::redundant_pub_crate)]
pub(crate) struct ThemeEditor {
    fixed: Entity<SelectState<ThemeOptions>>,
    light: Entity<SelectState<ThemeOptions>>,
    dark: Entity<SelectState<ThemeOptions>>,
    system_mode: ThemeMode,
}

impl ThemeEditor {
    pub(super) fn new(window: &mut Window, cx: &mut Context<'_, RopyBoard>) -> Self {
        Self {
            fixed: build_select(ThemeTarget::Fixed, window, cx),
            light: build_select(ThemeTarget::Light, window, cx),
            dark: build_select(ThemeTarget::Dark, window, cx),
            system_mode: window.appearance().into(),
        }
    }

    pub(crate) const fn select(&self, target: ThemeTarget) -> &Entity<SelectState<ThemeOptions>> {
        match target {
            ThemeTarget::Fixed => &self.fixed,
            ThemeTarget::Light => &self.light,
            ThemeTarget::Dark => &self.dark,
        }
    }

    pub(crate) const fn system_mode(&self) -> ThemeMode {
        self.system_mode
    }

    pub(crate) fn update_system_mode(&mut self, mode: ThemeMode) -> bool {
        let changed = self.system_mode != mode;
        self.system_mode = mode;
        changed
    }

    pub(crate) fn sync(&self, window: &mut Window, cx: &mut Context<'_, RopyBoard>) {
        for target in [ThemeTarget::Fixed, ThemeTarget::Light, ThemeTarget::Dark] {
            let id = GlobalSettings::read(cx, |settings| {
                target.selected(
                    settings,
                    ThemeCatalog::global(cx),
                    window.appearance().into(),
                )
            });
            let items = theme_options(target, cx);
            self.select(target).update(cx, |state, cx| {
                state.set_items(ThemeOptions(items), window, cx);
                state.set_selected_value(&ThemeChoice::Theme(id), window, cx);
            });
        }
    }
}

fn build_select(
    target: ThemeTarget,
    window: &mut Window,
    cx: &mut Context<'_, RopyBoard>,
) -> Entity<SelectState<ThemeOptions>> {
    let items = theme_options(target, cx);
    let selected = GlobalSettings::read(cx, |settings| {
        target.selected(
            settings,
            ThemeCatalog::global(cx),
            window.appearance().into(),
        )
    });
    let index = items
        .iter()
        .position(|item| item.id == ThemeChoice::Theme(selected.clone()))
        .map(|ix| IndexPath::default().row(ix));
    let select = cx.new(|cx| SelectState::new(ThemeOptions(items), index, window, cx));
    cx.subscribe_in(
        &select,
        window,
        move |board, _, event: &SelectEvent<ThemeOptions>, window, cx| match event {
            SelectEvent::Confirm(Some(ThemeChoice::Theme(id))) => {
                board.save_selected_theme(target, id, window, cx);
            }
            SelectEvent::Confirm(Some(ThemeChoice::Customize)) => {
                board.settings_editor.theme.sync(window, cx);
                board.open_themes(window, cx);
            }
            SelectEvent::Confirm(None) => {}
        },
    )
    .detach();
    select
}

fn theme_options(target: ThemeTarget, cx: &App) -> Vec<ThemeOption> {
    ThemeCatalog::global(cx)
        .definitions()
        .filter_map(|definition| {
            if target.mode().is_some_and(|mode| definition.mode() != mode) {
                return None;
            }
            let id = definition.id().clone();
            let title = definition.name().to_owned();
            let palette = definition.palette();
            Some(ThemeOption {
                title: title.into(),
                id: ThemeChoice::Theme(id),
                swatch: [palette.background, palette.selection, palette.primary],
            })
        })
        .chain(std::iter::once(ThemeOption {
            id: ThemeChoice::Customize,
            title: rust_i18n::t!("themes_customize").into(),
            swatch: [0; 3],
        }))
        .collect()
}

/// Use the delegate hook so the command can fill the row instead of the
/// default adapter's intrinsic-width label wrapper.
#[expect(clippy::redundant_pub_crate)]
pub(crate) struct ThemeOptions(Vec<ThemeOption>);

impl SelectDelegate for ThemeOptions {
    type Item = ThemeOption;

    fn items_count(&self, section: usize) -> usize {
        self.0.items_count(section)
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.0.item(ix)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SelectItem<Value = V>,
        V: PartialEq,
    {
        self.0.position(value)
    }

    fn render_item(
        &self,
        _: IndexPath,
        item: &Self::Item,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        (item.id == ThemeChoice::Customize).then(|| item.content())
    }
}
