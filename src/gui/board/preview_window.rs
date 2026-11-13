//! Hold-key inspection in a native window. The board owns its lifetime; the
//! preview owns content, scrolling and image geometry, and only requests closure.

mod image_viewport;

use std::{path::PathBuf, sync::Arc, time::Duration};

use gpui_kit::{
    AnyElement, AnyWindowHandle, App, AppContext, AsKeystroke, Bounds, Context, FocusHandle,
    KeyDownEvent, KeyUpEvent, ListAlignment, ListState, MouseButton, Pixels, Point, Render,
    SharedString, Size, StyledImage, Subscription, Task, WeakEntity, Window, WindowBounds,
    WindowKind, WindowOptions,
    base::TestSupportExt,
    component::{
        ActiveTheme, Disableable, IconName, Sizable, WindowExt,
        button::{Button, ButtonVariants},
        scroll::Scrollbar,
        spinner::Spinner,
    },
    div, img, list, point,
    prelude::{
        FluentBuilder, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
        Styled,
    },
    px, size,
};
use rust_i18n::t;

use super::{ActivePanel, RopyBoard, actions::PreviewSelection};
use crate::{
    repository::{ClipboardRecord, ContentType, deserialize_file_paths},
    utils::read_or_recover,
};
use image_viewport::ImageViewport;

#[derive(Default, PartialEq, Eq)]
pub(super) enum PreviewKeyState {
    #[default]
    Released,
    Held,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PreviewMode {
    Hold,
    Hover,
}

#[derive(Default)]
pub(super) struct HoverPreviewState {
    hovered_record_id: Option<u64>,
    anchor_y: Option<Pixels>,
    open_task: Option<Task<()>>,
    close_task: Option<Task<()>>,
    preview_hovered: bool,
}

impl HoverPreviewState {
    fn reset(&mut self) {
        self.hovered_record_id = None;
        self.anchor_y = None;
        self.open_task = None;
        self.close_task = None;
        self.preview_hovered = false;
    }
}

pub(super) struct PreviewSession {
    handle: AnyWindowHandle,
    parent: AnyWindowHandle,
    view: WeakEntity<PreviewWindow>,
    mode: PreviewMode,
    bounds: Bounds<Pixels>,
    displayed_record_id: u64,
    _closed: Subscription,
    _owner_released: Subscription,
    focus_check: Option<Task<()>>,
}

const PREVIEW_GAP_PX: f32 = 10.0;
const PREVIEW_MIN_SIDE_WIDTH_PX: f32 = 240.0;
const PREVIEW_CHROME_HEIGHT_PX: f32 = 64.0;
const PREVIEW_TEXT_WIDTH_PX: f32 = 380.0;
const PREVIEW_TEXT_MIN_HEIGHT_PX: f32 = 220.0;
const PREVIEW_TEXT_MAX_HEIGHT_PX: f32 = 440.0;
const PREVIEW_FILE_WIDTH_PX: f32 = 420.0;
const PREVIEW_FILE_MIN_HEIGHT_PX: f32 = 200.0;
const PREVIEW_FILE_MAX_HEIGHT_PX: f32 = 380.0;
const PREVIEW_IMAGE_MIN_CANVAS_WIDTH_PX: f32 = 260.0;
const PREVIEW_IMAGE_MIN_CANVAS_HEIGHT_PX: f32 = 160.0;
const PREVIEW_IMAGE_MAX_CANVAS_WIDTH_PX: f32 = 500.0;
const PREVIEW_IMAGE_MAX_CANVAS_HEIGHT_PX: f32 = 420.0;
const PREVIEW_IMAGE_FALLBACK_WIDTH_PX: f32 = 380.0;
const PREVIEW_IMAGE_FALLBACK_HEIGHT_PX: f32 = 320.0;

fn preview_target_size(record: &ClipboardRecord, scale_factor: f32) -> Size<Pixels> {
    match record.content_type {
        ContentType::Image => {
            let dims = image::image_dimensions(&record.content).ok();
            preview_image_window_size(dims, scale_factor)
        }
        ContentType::FilePath => {
            let count = deserialize_file_paths(&record.content).len().max(1);
            #[expect(clippy::cast_precision_loss, reason = "file count fits UI geometry")]
            let height = (count as f32)
                .mul_add(20.0, 24.0 + PREVIEW_CHROME_HEIGHT_PX)
                .clamp(PREVIEW_FILE_MIN_HEIGHT_PX, PREVIEW_FILE_MAX_HEIGHT_PX);
            size(px(PREVIEW_FILE_WIDTH_PX), px(height))
        }
        ContentType::Text | ContentType::RichText => {
            let estimated_lines: usize = record
                .content
                .lines()
                .map(|line| (line.chars().count() / 45).max(1))
                .sum::<usize>()
                .clamp(1, 100);
            #[expect(
                clippy::cast_precision_loss,
                reason = "capped line count fits UI geometry"
            )]
            let height = (estimated_lines as f32)
                .mul_add(18.0, 24.0 + PREVIEW_CHROME_HEIGHT_PX)
                .clamp(PREVIEW_TEXT_MIN_HEIGHT_PX, PREVIEW_TEXT_MAX_HEIGHT_PX);
            size(px(PREVIEW_TEXT_WIDTH_PX), px(height))
        }
    }
}

fn preview_image_window_size(dimensions: Option<(u32, u32)>, scale_factor: f32) -> Size<Pixels> {
    let Some((width, height)) = dimensions.filter(|(w, h)| *w > 0 && *h > 0) else {
        return size(
            px(PREVIEW_IMAGE_FALLBACK_WIDTH_PX),
            px(PREVIEW_IMAGE_FALLBACK_HEIGHT_PX),
        );
    };
    let scale_factor = scale_factor.max(1.0);
    #[expect(
        clippy::cast_precision_loss,
        reason = "raster dimensions fit UI geometry"
    )]
    let (logical_w, logical_h) = (width as f32 / scale_factor, height as f32 / scale_factor);
    let fit_scale = (PREVIEW_IMAGE_MAX_CANVAS_WIDTH_PX / logical_w)
        .min(PREVIEW_IMAGE_MAX_CANVAS_HEIGHT_PX / logical_h)
        .min(1.0);
    let canvas_w = (logical_w * fit_scale).max(PREVIEW_IMAGE_MIN_CANVAS_WIDTH_PX);
    let canvas_h = (logical_h * fit_scale).max(PREVIEW_IMAGE_MIN_CANVAS_HEIGHT_PX);
    size(px(canvas_w), px(canvas_h + PREVIEW_CHROME_HEIGHT_PX))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidePreference {
    Right,
    Left,
}

fn preview_bounds(
    work_area: Bounds<Pixels>,
    parent_bounds: Bounds<Pixels>,
    desired_size: Size<Pixels>,
    anchor_y: Option<Pixels>,
    side_preference: SidePreference,
) -> Bounds<Pixels> {
    let gap = px(PREVIEW_GAP_PX);
    let desired_w = desired_size.width.min(work_area.size.width);
    let height = desired_size.height.min(work_area.size.height);
    let min_side_w = px(PREVIEW_MIN_SIDE_WIDTH_PX).min(desired_w);

    let right_space = (work_area.right() - parent_bounds.right() - gap).max(px(0.));
    let left_space = (parent_bounds.left() - work_area.left() - gap).max(px(0.));

    let (x, width) = match side_preference {
        SidePreference::Right => {
            if right_space >= desired_w {
                (parent_bounds.right() + gap, desired_w)
            } else if left_space >= desired_w {
                (parent_bounds.left() - gap - desired_w, desired_w)
            } else if right_space >= left_space && right_space >= min_side_w {
                (parent_bounds.right() + gap, right_space)
            } else if left_space >= min_side_w {
                (parent_bounds.left() - gap - left_space, left_space)
            } else {
                let ideal_x = parent_bounds.center().x - desired_w / 2.;
                let min_x = work_area.left();
                let max_x = (work_area.right() - desired_w).max(min_x);
                (ideal_x.clamp(min_x, max_x), desired_w)
            }
        }
        SidePreference::Left => {
            if left_space >= desired_w {
                (parent_bounds.left() - gap - desired_w, desired_w)
            } else if right_space >= desired_w {
                (parent_bounds.right() + gap, desired_w)
            } else if left_space >= right_space && left_space >= min_side_w {
                (parent_bounds.left() - gap - left_space, left_space)
            } else if right_space >= min_side_w {
                (parent_bounds.right() + gap, right_space)
            } else {
                let ideal_x = parent_bounds.center().x - desired_w / 2.;
                let min_x = work_area.left();
                let max_x = (work_area.right() - desired_w).max(min_x);
                (ideal_x.clamp(min_x, max_x), desired_w)
            }
        }
    };

    let min_y = work_area.top();
    let max_y = (work_area.bottom() - height).max(min_y);
    let y = anchor_y
        .unwrap_or_else(|| parent_bounds.top())
        .clamp(min_y, max_y);

    Bounds::new(point(x, y), size(width, height))
}

pub(super) fn is_preview_key(key: &str, cx: &App) -> bool {
    cx.key_bindings()
        .borrow()
        .bindings_for_action(&PreviewSelection)
        .any(|binding| {
            binding
                .keystrokes()
                .last()
                .is_some_and(|stroke| stroke.as_keystroke().key == key)
        })
}

const PREVIEW_HOVER_OPEN_DELAY: Duration = Duration::from_millis(350);
const PREVIEW_HOVER_GRID_SWITCH_DELAY: Duration = Duration::from_millis(250);
const PREVIEW_HOVER_CLOSE_DELAY: Duration = Duration::from_millis(200);

impl RopyBoard {
    pub(super) fn is_hold_preview_active(&self) -> bool {
        self.preview_window
            .as_ref()
            .is_some_and(|session| session.mode == PreviewMode::Hold)
    }

    pub(super) fn is_hover_preview_active(&self) -> bool {
        self.preview_window
            .as_ref()
            .is_some_and(|session| session.mode == PreviewMode::Hover)
    }

    pub(super) fn cancel_hover_preview(&mut self, cx: &mut Context<'_, Self>) {
        self.hover_preview.reset();
        if self.is_hover_preview_active() {
            self.dismiss_preview(cx);
        }
    }

    pub(super) fn on_record_hover(
        &mut self,
        record_id: u64,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if hovered {
            if !self.settings_editor.panel_state.hover_preview_enabled
                || self.ui_state.any_overlay_visible()
                || window.has_active_dialog(cx)
                || self.active_panel != ActivePanel::ClipboardList
                || self.is_hold_preview_active()
            {
                return;
            }
            self.hover_preview.close_task = None;
            self.hover_preview.hovered_record_id = Some(record_id);
            let parent_top = window.bounds().top();
            self.hover_preview.anchor_y =
                Some((parent_top + window.mouse_position().y - px(20.)).max(parent_top));

            if self.is_hover_preview_active() {
                if self.layout_mode == crate::config::LayoutMode::Grid {
                    self.hover_preview.open_task =
                        Some(cx.spawn_in(window, async move |this, cx| {
                            cx.background_executor()
                                .timer(PREVIEW_HOVER_GRID_SWITCH_DELAY)
                                .await;
                            let _ = this.update_in(cx, |board, window, cx| {
                                if board.hover_preview.hovered_record_id == Some(record_id)
                                    && board.is_hover_preview_active()
                                {
                                    board.update_hover_preview_record(record_id, window, cx);
                                }
                            });
                        }));
                } else {
                    self.hover_preview.open_task = None;
                    self.update_hover_preview_record(record_id, window, cx);
                }
                return;
            }

            self.hover_preview.open_task = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(PREVIEW_HOVER_OPEN_DELAY)
                    .await;
                let _ = this.update_in(cx, |board, window, cx| {
                    if board.hover_preview.hovered_record_id != Some(record_id)
                        || board.preview_window.is_some()
                        || !board.settings_editor.panel_state.hover_preview_enabled
                        || board.ui_state.any_overlay_visible()
                        || window.has_active_dialog(cx)
                        || board.active_panel != ActivePanel::ClipboardList
                    {
                        return;
                    }
                    let record = read_or_recover(&board.records)
                        .iter()
                        .find(|r| r.id == record_id)
                        .cloned();
                    if let Some(record) = record {
                        board.open_preview_for_record(record, PreviewMode::Hover, window, cx);
                    }
                });
            }));
        } else if self.hover_preview.hovered_record_id == Some(record_id) {
            self.hover_preview.hovered_record_id = None;
            self.hover_preview.open_task = None;
            if self.is_hover_preview_active() && !self.hover_preview.preview_hovered {
                self.schedule_hover_close(window, cx);
            }
        }
    }

    pub(super) fn on_preview_hover(
        &mut self,
        hovered: bool,
        window: &Window,
        cx: &Context<'_, Self>,
    ) {
        if !self.is_hover_preview_active() {
            return;
        }
        self.hover_preview.preview_hovered = hovered;
        if hovered {
            self.hover_preview.close_task = None;
        } else if self.hover_preview.hovered_record_id.is_none() {
            self.schedule_hover_close(window, cx);
        }
    }

    fn schedule_hover_close(&mut self, window: &Window, cx: &Context<'_, Self>) {
        self.hover_preview.close_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(PREVIEW_HOVER_CLOSE_DELAY)
                .await;
            let _ = this.update(cx, |board, cx| {
                if board.is_hover_preview_active()
                    && board.hover_preview.hovered_record_id.is_none()
                    && !board.hover_preview.preview_hovered
                {
                    board.dismiss_preview(cx);
                }
            });
        }));
    }

    fn filtered_index_for_record_id(&self, record_id: u64) -> Option<usize> {
        let records = read_or_recover(&self.records);
        self.filtered_record_indices
            .iter()
            .position(|&ix| records.get(ix).is_some_and(|r| r.id == record_id))
    }

    fn preview_anchor(
        &self,
        record_index: Option<usize>,
        fallback_y: Option<Pixels>,
        window: &Window,
    ) -> (Option<Pixels>, SidePreference) {
        let parent_top = window.bounds().top();
        let parent_bottom = window.bounds().bottom();
        match self.layout_mode {
            crate::config::LayoutMode::Grid => {
                let viewport = self.grid_scroll_handle.bounds();
                if viewport.size.height <= px(0.) {
                    return (fallback_y, SidePreference::Right);
                }
                let Some(ix) = record_index else {
                    return (fallback_y, SidePreference::Right);
                };
                let layout = self
                    .grid_layout
                    .layout(&self.filtered_record_indices, &self.records);
                let Some(card) = layout.bounds.get(ix) else {
                    return (fallback_y, SidePreference::Right);
                };
                let y =
                    (parent_top + viewport.top() + card.top() + self.grid_scroll_handle.offset().y)
                        .clamp(parent_top, parent_bottom);
                let side = if card.center().x < viewport.size.width / 2. {
                    SidePreference::Left
                } else {
                    SidePreference::Right
                };
                (Some(y), side)
            }
            crate::config::LayoutMode::List => {
                if fallback_y.is_some() {
                    return (fallback_y, SidePreference::Right);
                }
                let Some(ix) = record_index else {
                    return (None, SidePreference::Right);
                };
                let top = self.list_state.logical_scroll_top();
                if ix <= top.item_ix {
                    return (None, SidePreference::Right);
                }
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "visible row offset fits UI geometry"
                )]
                let row_offset = (ix - top.item_ix) as f32 * px(72.) - top.offset_in_item;
                let y = (parent_top + px(96.) + row_offset).clamp(parent_top, parent_bottom);
                (Some(y), SidePreference::Right)
            }
        }
    }

    fn update_hover_preview_record(
        &mut self,
        record_id: u64,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(session) = self.preview_window.as_ref() else {
            return;
        };
        if session.mode != PreviewMode::Hover || session.displayed_record_id == record_id {
            return;
        }
        let record = read_or_recover(&self.records)
            .iter()
            .find(|r| r.id == record_id)
            .cloned();
        let (Some(record), Some(display)) = (record, window.display(cx)) else {
            return;
        };
        let desired_size = preview_target_size(&record, window.scale_factor());
        let (anchor_y, side_preference) = self.preview_anchor(
            self.filtered_index_for_record_id(record_id),
            self.hover_preview.anchor_y,
            window,
        );
        let next_bounds = preview_bounds(
            display.visible_bounds(),
            window.bounds(),
            desired_size,
            anchor_y,
            side_preference,
        );
        let current_bounds = session.bounds;
        let handle = session.handle;
        let view = session.view.clone();

        // Reopen if origin must shift (e.g. left-docked window growing wider).
        if (next_bounds.origin.x - current_bounds.origin.x).abs() > px(0.5)
            || (next_bounds.origin.y - current_bounds.origin.y).abs() > px(0.5)
        {
            self.dismiss_preview(cx);
            self.open_preview_for_record(record, PreviewMode::Hover, window, cx);
            return;
        }

        let updated = handle
            .update(cx, |_, preview_window, cx| {
                if next_bounds.size != current_bounds.size {
                    preview_window.resize(next_bounds.size);
                }
                let _ = view.update(cx, |preview, cx| preview.set_record(record, cx));
            })
            .is_ok();
        if updated && let Some(session) = self.preview_window.as_mut() {
            session.bounds = next_bounds;
            session.displayed_record_id = record_id;
        }
    }

    pub(super) fn open_preview(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.is_hover_preview_active() {
            self.dismiss_preview(cx);
        }
        if self.preview_key == PreviewKeyState::Held
            || self.preview_window.is_some()
            || !self.settings_editor.panel_state.space_preview_enabled
            || self.active_panel != ActivePanel::ClipboardList
            || window.has_active_dialog(cx)
            || !self.focus_handle.is_focused(window)
        {
            return;
        }
        let record = {
            let records = read_or_recover(&self.records);
            self.filtered_record_indices
                .get(self.selected_index)
                .and_then(|ix| records.get(*ix))
                .cloned()
        };
        let Some(record) = record else {
            return;
        };
        self.open_preview_for_record(record, PreviewMode::Hold, window, cx);
    }

    fn open_preview_for_record(
        &mut self,
        record: ClipboardRecord,
        mode: PreviewMode,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(display) = window.display(cx) else {
            return;
        };
        let parent = window.window_handle();
        let owner = cx.weak_entity();
        let record_id = record.id;
        let desired_size = preview_target_size(&record, window.scale_factor());
        let (record_index, fallback_y) = match mode {
            PreviewMode::Hold => (Some(self.selected_index), None),
            PreviewMode::Hover => (
                self.filtered_index_for_record_id(record_id),
                self.hover_preview.anchor_y,
            ),
        };
        let (anchor_y, side_preference) = self.preview_anchor(record_index, fallback_y, window);
        let bounds = preview_bounds(
            display.visible_bounds(),
            window.bounds(),
            desired_size,
            anchor_y,
            side_preference,
        );
        let result = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: Some(display.id()),
                // Match the main popup's level; Floating is below it on macOS.
                kind: WindowKind::PopUp,
                titlebar: None,
                focus: false,
                is_movable: false,
                is_minimizable: false,
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| PreviewWindow::new(owner, parent, mode, record, window, cx)),
        );
        match result {
            Ok((handle, entity)) => {
                let owner = cx.weak_entity();
                let closed = cx.on_window_closed(move |cx, id| {
                    if id == parent.window_id() || id == handle.window_id() {
                        let owner = owner.clone();
                        // Window removal can synchronously notify while the board
                        // is still leased by close_preview.
                        cx.defer(move |cx| {
                            let _ = owner.update(cx, |board, cx| {
                                if board
                                    .preview_window
                                    .as_ref()
                                    .is_some_and(|session| session.handle == handle)
                                {
                                    board.close_preview(cx);
                                }
                            });
                        });
                    }
                });
                let owner_released = cx.on_release(move |_, cx| {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                });
                if mode == PreviewMode::Hold {
                    self.preview_key = PreviewKeyState::Held;
                } else {
                    window.activate_window();
                }
                self.preview_window = Some(PreviewSession {
                    handle,
                    parent,
                    view: entity.downgrade(),
                    mode,
                    bounds,
                    displayed_record_id: record_id,
                    _closed: closed,
                    _owner_released: owner_released,
                    focus_check: None,
                });
                cx.notify();
            }
            Err(error) => {
                tracing::error!(%error, "failed to open preview");
                if mode == PreviewMode::Hold {
                    window.push_notification(t!("preview_open_failed").to_string(), cx);
                }
            }
        }
    }

    pub(super) fn close_preview(&mut self, cx: &mut Context<'_, Self>) -> bool {
        self.remove_preview(true, cx)
    }

    pub(super) fn dismiss_preview(&mut self, cx: &mut Context<'_, Self>) {
        self.remove_preview(false, cx);
    }

    fn remove_preview(&mut self, restore_parent: bool, cx: &mut Context<'_, Self>) -> bool {
        let Some(session) = self.preview_window.take() else {
            return false;
        };
        if session.mode == PreviewMode::Hover {
            self.hover_preview.open_task = None;
            self.hover_preview.close_task = None;
            self.hover_preview.preview_hovered = false;
        }
        let restore_focus = session
            .handle
            .update(cx, |_, window, _| {
                let active = window.is_window_active();
                window.remove_window();
                restore_parent && active
            })
            .unwrap_or(false);
        if restore_focus {
            // Do not use popup activation: it resets search, selection and geometry.
            cx.defer(move |cx| {
                let _ = session
                    .parent
                    .update(cx, |_, window, _| window.activate_window());
            });
        }
        cx.notify();
        true
    }

    pub(super) fn defer_preview_focus_check(&mut self, window: &Window, cx: &Context<'_, Self>) {
        let Some(session) = self.preview_window.as_mut() else {
            return;
        };
        // Native main-window deactivation arrives before preview activation.
        // Allow the OS focus transition to settle; releasing the key still closes
        // immediately. Owning the task in the session cancels stale checks.
        session.focus_check = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(75))
                .await;
            let _ = this.update_in(cx, |board, window, cx| {
                let Some(session) = board.preview_window.as_ref() else {
                    return;
                };
                // On macOS App::active_window uses NSApplication.mainWindow,
                // which excludes popup panels even when they have keyboard focus.
                let preview_active = session
                    .handle
                    .update(cx, |_, preview, _| preview.is_window_active())
                    .unwrap_or(false);
                if preview_active || crate::gui::window::has_native_focus(window) {
                    return;
                }
                board.preview_key = PreviewKeyState::Released;
                board.close_preview(cx);
                board.cancel_shortcut_recording(cx);
                if Self::should_auto_hide_on_focus_out(board.pinned)
                    && !window.has_active_dialog(cx)
                {
                    board.invalidate_copy_confirmation();
                    board.clear_search(window, cx);
                    crate::gui::hide_window(window, cx, board.pinned);
                }
            });
        }));
    }
}

enum Content {
    Text {
        title: SharedString,
        details: SharedString,
        lines: Arc<Vec<SharedString>>,
        scroll: ListState,
    },
    Image {
        path: PathBuf,
        load: ImageLoad,
    },
}

#[derive(Clone, Copy)]
enum ImageLoad {
    Loading,
    Ready(Size<Pixels>),
    Failed,
}

struct PreviewWindow {
    owner: WeakEntity<RopyBoard>,
    parent: AnyWindowHandle,
    mode: PreviewMode,
    focus: FocusHandle,
    content: Content,
    image: ImageViewport,
    drag_origin: Option<(Point<Pixels>, Point<Pixels>)>,
    load_task: Option<Task<()>>,
    _activation: Subscription,
}

impl PreviewWindow {
    fn new(
        owner: WeakEntity<RopyBoard>,
        parent: AnyWindowHandle,
        mode: PreviewMode,
        record: ClipboardRecord,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        if mode == PreviewMode::Hold {
            window.focus(&focus, cx);
        }
        window.set_window_title(&t!("preview_title"));
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                let owner = this.owner.clone();
                let parent = this.parent;
                cx.defer(move |cx| {
                    let _ = parent.update(cx, |_, window, cx| {
                        let _ = owner
                            .update(cx, |board, cx| board.defer_preview_focus_check(window, cx));
                    });
                });
            }
        });
        let (content, load) = Self::build_content(record, cx);
        Self {
            owner,
            parent,
            mode,
            focus,
            content,
            image: ImageViewport::default(),
            drag_origin: None,
            load_task: load,
            _activation: activation,
        }
    }

    fn set_record(&mut self, record: ClipboardRecord, cx: &mut Context<'_, Self>) {
        let (content, load) = Self::build_content(record, cx);
        self.content = content;
        self.load_task = load;
        self.image = ImageViewport::default();
        self.drag_origin = None;
        cx.notify();
    }

    fn build_content(
        record: ClipboardRecord,
        cx: &Context<'_, Self>,
    ) -> (Content, Option<Task<()>>) {
        let mut load = None;
        if record.content_type == ContentType::Image {
            let path = PathBuf::from(record.content);
            let source = path.clone();
            let task = cx.background_spawn(async move { image::image_dimensions(source) });
            load = Some(cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    if let Content::Image { load, .. } = &mut this.content {
                        match result {
                            Ok((width, height)) if width > 0 && height > 0 => {
                                #[expect(
                                    clippy::cast_precision_loss,
                                    reason = "raster dimensions fit UI geometry"
                                )]
                                {
                                    *load =
                                        ImageLoad::Ready(size(px(width as f32), px(height as f32)));
                                }
                            }
                            _ => *load = ImageLoad::Failed,
                        }
                        cx.notify();
                    }
                });
            }));
            (
                Content::Image {
                    path,
                    load: ImageLoad::Loading,
                },
                load,
            )
        } else {
            let (text, title, details) = if record.content_type == ContentType::FilePath {
                let paths = deserialize_file_paths(&record.content);
                let details = t!("record_file_count", count = paths.len()).into_owned();
                (paths.join("\n"), t!("preview_files"), details)
            } else {
                let details =
                    t!("preview_characters", count = record.content.chars().count()).into_owned();
                (record.content, t!("preview_text"), details)
            };
            let lines: Vec<SharedString> = text
                .split('\n')
                .map(|line| {
                    if line.is_empty() {
                        SharedString::from(" ")
                    } else {
                        SharedString::from(line.to_owned())
                    }
                })
                .collect();
            let scroll = ListState::new(lines.len(), ListAlignment::Top, px(200.));
            (
                Content::Text {
                    title: title.into_owned().into(),
                    details: details.into(),
                    lines: Arc::new(lines),
                    scroll,
                },
                load,
            )
        }
    }

    fn request_close(&self, window: &Window, cx: &mut Context<'_, Self>) {
        let owner = self.owner.clone();
        let handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |board, cx| {
                // A delayed callback from a previous preview must not close a new one.
                if board
                    .preview_window
                    .as_ref()
                    .is_some_and(|session| session.handle == handle)
                {
                    board.close_preview(cx);
                }
            });
        });
    }

    fn render_content(&self, cx: &Context<'_, Self>) -> AnyElement {
        match &self.content {
            Content::Text { lines, scroll, .. } => {
                let lines = lines.clone();
                div()
                    .id("preview-text")
                    .test_support()
                    .relative()
                    .size_full()
                    .child(
                        list(scroll.clone(), move |ix, _, _| {
                            div()
                                .id(("preview-line", ix))
                                .test_support()
                                .aria_label(lines[ix].clone())
                                .px_4()
                                .when(ix == 0, Styled::pt_3)
                                .when(ix + 1 == lines.len(), Styled::pb_3)
                                .min_w_0()
                                .text_xs()
                                .line_height(gpui_kit::relative(1.45))
                                .child(lines[ix].clone())
                                .into_any_element()
                        })
                        .size_full(),
                    )
                    .child(Scrollbar::vertical(scroll))
                    .into_any_element()
            }
            Content::Image { path, load } => {
                let source = path.clone();
                let load = *load;
                // Canvas uses resolved geometry so header/font scaling and resizing
                // cannot desynchronize pointer coordinates from raster placement.
                gpui_kit::canvas(
                    cx.processor(move |this, bounds, window, cx| {
                        let previous_scale = this.image.scale();
                        this.image.set_bounds(bounds);
                        let child = if let ImageLoad::Ready(dimensions) = load {
                            // Original size means one raster pixel per display pixel,
                            // including Retina/HiDPI windows.
                            this.image.set_dimensions(
                                dimensions.map(|edge| edge / window.scale_factor()),
                            );
                            // The first layout and native resizes resolve the fit scale here.
                            // Refresh the footer only when that resolved scale changes.
                            if (this.image.scale() - previous_scale).abs() > f32::EPSILON {
                                window.request_animation_frame();
                            }
                            let placement = this.image.placement();
                            div()
                                .size_full()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .id("preview-image")
                                        .test_support()
                                        .absolute()
                                        .left(placement.origin.x)
                                        .top(placement.origin.y)
                                        .w(placement.size.width)
                                        .h(placement.size.height)
                                        .child(
                                            img(source.clone())
                                                .size_full()
                                                .with_loading(|| Spinner::new().into_any_element())
                                                .with_fallback(|| {
                                                    div()
                                                        .child(
                                                            t!("preview_unavailable").to_string(),
                                                        )
                                                        .into_any_element()
                                                }),
                                        ),
                                )
                                .into_any_element()
                        } else if matches!(load, ImageLoad::Failed) {
                            div()
                                .child(t!("preview_unavailable").to_string())
                                .into_any_element()
                        } else {
                            Spinner::new().into_any_element()
                        };
                        let mut child = child;
                        child.layout_as_root(
                            bounds.size.map(gpui_kit::AvailableSpace::Definite),
                            window,
                            cx,
                        );
                        child.prepaint_at(bounds.origin, window, cx);
                        child
                    }),
                    |_, mut child, window, cx| child.paint(window, cx),
                )
                .size_full()
                .into_any_element()
            }
        }
    }
}

impl PreviewWindow {
    fn render_header(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let (title, details): (SharedString, SharedString) = match &self.content {
            Content::Text { title, details, .. } => (title.clone(), details.clone()),
            Content::Image { load, .. } => {
                let details = match load {
                    ImageLoad::Ready(dimensions) => t!(
                        "preview_dimensions",
                        width = format!("{:.0}", f32::from(dimensions.width)),
                        height = format!("{:.0}", f32::from(dimensions.height))
                    )
                    .into_owned(),
                    ImageLoad::Loading | ImageLoad::Failed => String::new(),
                };
                (t!("preview_image").into_owned().into(), details.into())
            }
        };
        div()
            .id("preview-header")
            .test_support()
            .flex()
            .items_center()
            .flex_shrink_0()
            .h_8()
            .px_3()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .text_xs()
            .child(div().font_weight(gpui_kit::FontWeight::MEDIUM).child(title))
            .child(
                div()
                    .id("preview-details")
                    .test_support()
                    .aria_label(details.clone())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(details),
            )
            .child(
                Button::new("preview-close")
                    .small()
                    .ghost()
                    .icon(IconName::Close)
                    .accessibility_label(t!("preview_close"))
                    .tooltip(t!("preview_close"))
                    .on_click(cx.listener(|this, _, window, cx| this.request_close(window, cx))),
            )
    }

    fn render_footer(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let image_content = matches!(self.content, Content::Image { .. });
        let image_ready = matches!(
            self.content,
            Content::Image {
                load: ImageLoad::Ready(_),
                ..
            }
        );
        let scale = t!(
            "preview_scale",
            percent = format!("{:.0}", self.image.scale() * 100.)
        )
        .into_owned();
        div()
            .id("preview-footer")
            .test_support()
            .flex()
            .items_center()
            .flex_shrink_0()
            .h_8()
            .px_3()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .text_size(px(11.))
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(self.mode == PreviewMode::Hold, |el| {
                        el.child(t!("preview_release_to_close").into_owned())
                    }),
            )
            .when(image_ready, |footer| {
                footer.child(
                    div()
                        .id("preview-scale")
                        .test_support()
                        .aria_label(scale.clone())
                        .child(scale),
                )
            })
            .when(image_content, |footer| {
                footer
                    .child(
                        Button::new("preview-fit")
                            .small()
                            .ghost()
                            .label(t!("preview_fit"))
                            .disabled(!image_ready)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.image.reset();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("preview-original")
                            .small()
                            .ghost()
                            .label(t!("preview_actual_size"))
                            .accessibility_label(t!("preview_original"))
                            .tooltip(t!("preview_original"))
                            .disabled(!image_ready)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.image.original_size();
                                cx.notify();
                            })),
                    )
            })
    }
}

impl Render for PreviewWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let image_content = matches!(self.content, Content::Image { .. });
        div()
            .id("preview-root")
            .test_support()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .track_focus(&self.focus)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                let owner = this.owner.clone();
                let parent = this.parent;
                let hovered = *hovered;
                cx.defer(move |cx| {
                    let _ = parent.update(cx, |_, window, cx| {
                        let _ = owner.update(cx, |board, cx| {
                            board.on_preview_hover(hovered, window, cx);
                        });
                    });
                });
            }))
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                if is_preview_key(&event.keystroke.key, cx) {
                    let owner = this.owner.clone();
                    cx.defer(move |cx| {
                        let _ = owner
                            .update(cx, |board, _| board.preview_key = PreviewKeyState::Released);
                    });
                    this.request_close(window, cx);
                }
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.request_close(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.render_header(cx))
            .child(
                div()
                    .id("preview-canvas")
                    .test_support()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .when(image_content, |canvas| {
                        canvas
                            .bg(cx.theme().muted)
                            .on_scroll(cx.listener(
                                |this, event: &gpui_kit::ScrollEvent, window, cx| {
                                    this.image.zoom_at(
                                        event.position,
                                        f32::from(event.delta.pixel_delta(window.rem_size()).y),
                                    );
                                    cx.stop_propagation();
                                    cx.notify();
                                },
                            ))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &gpui_kit::MouseDownEvent, _, _| {
                                    this.drag_origin = Some((event.position, this.image.offset()));
                                }),
                            )
                            .on_mouse_move(cx.listener(
                                |this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                                    if event.pressed_button == Some(MouseButton::Left) {
                                        if let Some((origin, offset)) = this.drag_origin {
                                            this.image.pan(offset + event.position - origin);
                                            cx.notify();
                                        }
                                    } else {
                                        this.drag_origin = None;
                                    }
                                },
                            ))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| this.drag_origin = None),
                            )
                    })
                    .child(self.render_content(cx)),
            )
            .child(self.render_footer(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_bounds_prefers_right_side_without_overlap() {
        let work_area = Bounds::new(point(px(0.), px(25.)), size(px(1440.), px(875.)));
        let parent = Bounds::new(point(px(200.), px(120.)), size(px(400.), px(550.)));
        let desired = size(px(380.), px(320.));

        let bounds = preview_bounds(work_area, parent, desired, None, SidePreference::Right);
        assert_eq!(bounds.origin, point(px(610.), px(120.)));
        assert_eq!(bounds.size, desired);
    }

    #[test]
    fn test_preview_bounds_flips_to_left_when_right_space_insufficient() {
        let work_area = Bounds::new(point(px(0.), px(25.)), size(px(1440.), px(875.)));
        let parent = Bounds::new(point(px(950.), px(120.)), size(px(400.), px(550.)));
        let desired = size(px(380.), px(320.));

        let bounds = preview_bounds(work_area, parent, desired, None, SidePreference::Right);
        assert_eq!(bounds.origin, point(px(560.), px(120.)));
        assert_eq!(bounds.size, desired);
    }

    #[test]
    fn test_preview_bounds_prefers_left_when_requested_and_falls_back_to_right() {
        let work_area = Bounds::new(point(px(0.), px(25.)), size(px(1440.), px(875.)));
        let centered_parent = Bounds::new(point(px(520.), px(120.)), size(px(400.), px(550.)));
        let desired = size(px(380.), px(320.));
        assert_eq!(
            preview_bounds(
                work_area,
                centered_parent,
                desired,
                None,
                SidePreference::Left
            )
            .origin,
            point(px(130.), px(120.))
        );

        let left_edge_parent = Bounds::new(point(px(40.), px(120.)), size(px(400.), px(550.)));
        assert_eq!(
            preview_bounds(
                work_area,
                left_edge_parent,
                desired,
                None,
                SidePreference::Left
            )
            .origin,
            point(px(450.), px(120.))
        );
    }

    #[test]
    fn test_preview_bounds_shrinks_width_to_fit_wider_side_and_clamps_vertical() {
        let work_area = Bounds::new(point(px(0.), px(25.)), size(px(1000.), px(600.)));
        let parent = Bounds::new(point(px(550.), px(400.)), size(px(400.), px(550.)));
        let desired = size(px(600.), px(400.));

        let bounds = preview_bounds(work_area, parent, desired, None, SidePreference::Right);
        assert_eq!(bounds.origin, point(px(0.), px(225.)));
        assert_eq!(bounds.size, size(px(540.), px(400.)));
    }

    #[test]
    fn test_preview_bounds_falls_back_to_overlay_when_neither_side_has_min_width() {
        let work_area = Bounds::new(point(px(0.), px(25.)), size(px(600.), px(600.)));
        let parent = Bounds::new(point(px(100.), px(50.)), size(px(400.), px(550.)));
        let desired = size(px(380.), px(320.));

        let bounds = preview_bounds(work_area, parent, desired, None, SidePreference::Right);
        assert_eq!(bounds.origin, point(px(110.), px(50.)));
        assert_eq!(bounds.size, desired);
    }

    #[test]
    fn test_preview_image_window_size_adapts_to_aspect_ratio_and_clamps() {
        // Wide 2:1 image at 1x scale fits max canvas 500x420 -> 500x250 canvas + 64 chrome.
        assert_eq!(
            preview_image_window_size(Some((1200, 600)), 1.0),
            size(px(500.), px(314.))
        );
        // Tiny icon respects minimum canvas 260x160 -> 260x224.
        assert_eq!(
            preview_image_window_size(Some((64, 64)), 2.0),
            size(px(260.), px(224.))
        );
        // Missing dimensions use fallback.
        assert_eq!(
            preview_image_window_size(None, 1.0),
            size(
                px(PREVIEW_IMAGE_FALLBACK_WIDTH_PX),
                px(PREVIEW_IMAGE_FALLBACK_HEIGHT_PX)
            )
        );
    }
}
