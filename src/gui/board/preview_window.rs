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

pub(super) struct PreviewSession {
    handle: AnyWindowHandle,
    parent: AnyWindowHandle,
    _closed: Subscription,
    _owner_released: Subscription,
    focus_check: Option<Task<()>>,
}

fn preview_bounds(work_area: Bounds<Pixels>, parent_size: Size<Pixels>) -> Bounds<Pixels> {
    let extent = size(
        parent_size.width.min(work_area.size.width),
        parent_size.height.min(work_area.size.height),
    );
    Bounds::new(
        work_area.center() - point(extent.width / 2., extent.height / 2.),
        extent,
    )
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

impl RopyBoard {
    pub(super) fn open_preview(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
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
        let (Some(record), Some(display)) = (record, window.display(cx)) else {
            return;
        };
        let parent = window.window_handle();
        let owner = cx.weak_entity();
        let result = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(preview_bounds(
                    display.visible_bounds(),
                    window.bounds().size,
                ))),
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
            |window, cx| cx.new(|cx| PreviewWindow::new(owner, parent, record, window, cx)),
        );
        match result {
            Ok((handle, _)) => {
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
                self.preview_key = PreviewKeyState::Held;
                self.preview_window = Some(PreviewSession {
                    handle,
                    parent,
                    _closed: closed,
                    _owner_released: owner_released,
                    focus_check: None,
                });
                cx.notify();
            }
            Err(error) => {
                tracing::error!(%error, "failed to open preview");
                window.push_notification(t!("preview_open_failed").to_string(), cx);
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
    focus: FocusHandle,
    content: Content,
    image: ImageViewport,
    drag_origin: Option<(Point<Pixels>, Point<Pixels>)>,
    _load: Option<Task<()>>,
    _activation: Subscription,
}

impl PreviewWindow {
    fn new(
        owner: WeakEntity<RopyBoard>,
        parent: AnyWindowHandle,
        record: ClipboardRecord,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
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
        let mut load = None;
        let content = if record.content_type == ContentType::Image {
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
            Content::Image {
                path,
                load: ImageLoad::Loading,
            }
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
            Content::Text {
                title: title.into_owned().into(),
                details: details.into(),
                lines: Arc::new(lines),
                scroll,
            }
        };
        Self {
            owner,
            parent,
            focus,
            content,
            image: ImageViewport::default(),
            drag_origin: None,
            _load: load,
            _activation: activation,
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
                                .px_5()
                                .when(ix == 0, Styled::pt_4)
                                .when(ix + 1 == lines.len(), Styled::pb_4)
                                .min_w_0()
                                .line_height(gpui_kit::relative(1.5))
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
            .h_10()
            .px_3()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .text_sm()
            .child(div().font_weight(gpui_kit::FontWeight::MEDIUM).child(title))
            .child(
                div()
                    .id("preview-details")
                    .test_support()
                    .aria_label(details.clone())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
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
            .h_10()
            .px_3()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(t!("preview_release_to_close").into_owned()),
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
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .track_focus(&self.focus)
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
